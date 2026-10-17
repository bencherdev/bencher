import "@bencherdev/ui/styles.css";
import "../styles/console.css";
import "../reports/reports.css";
import { MemoryRouter, Route, createMemoryHistory } from "@solidjs/router";
import { type QueryClient, QueryClientProvider } from "@tanstack/solid-query";
import { Suspense } from "solid-js";
import { render } from "solid-js/web";
import { afterEach, beforeEach, expect, test } from "vitest";
import { page } from "vitest/browser";
import type { JsonConsoleRunSummary } from "../../types/bencher";
import { type Api, ApiError } from "../api";
import { createQueryClient } from "../cache";
import { ProjectContext } from "../project";
import { DEFAULT_SEARCH } from "../reports/search";
import { fakeApi, watchFallback } from "../reports/testing";
import { screenBatch } from "../reports/virtual";
import { type RunsBatch, batchKey } from "../runs/query";
import {
	cutCells,
	cutHeaders,
	likelyRun,
	likelyRuns,
	overrun,
	runFixture,
} from "../runs/testing";
import Runs from "./Runs";

// Relative to the router base, as the app routes it.
const PATH = "/hashbrown/runs";

let dispose: (() => void) | undefined;

/** The page under a Suspense boundary, as it sits under the layout's. */
const mount = (api: Api, client: QueryClient, path = PATH) => {
	const history = createMemoryHistory();
	history.set({ value: path });
	const root = document.createElement("div");
	root.className = "console";
	document.body.append(root);
	dispose = render(
		() => (
			<QueryClientProvider client={client}>
				<MemoryRouter
					history={history}
					root={(props) => (
						<ProjectContext.Provider value={{ api, slug: () => "hashbrown" }}>
							<Suspense fallback={<p>Suspended</p>}>{props.children}</Suspense>
						</ProjectContext.Provider>
					)}
				>
					<Route path="/:project/runs" component={Runs} />
				</MemoryRouter>
			</QueryClientProvider>
		),
		root,
	);
};

/** The production cache, which does not retry an API that refused. */
const client = () => createQueryClient(() => {});

/** The production cache, but a server error fails at once. */
const impatient = () => {
	const cache = client();
	cache.setDefaultOptions({
		queries: { ...cache.getDefaultOptions().queries, retry: false },
	});
	return cache;
};

let perPage = 0;

const runs = (from: number, count: number) =>
	Array.from({ length: count }, (_, index) => runFixture(from + index));

/** What the runs endpoint answers: a page of runs and how many match. */
const answered = (page: JsonConsoleRunSummary[], total: number) => ({
	data: { runs: page, total },
});

const batchOf = (page: number, total: number): RunsBatch => ({
	runs: runs((page - 1) * perPage, perPage),
	total,
	batch: { page, perPage },
});

const cached = (cache: QueryClient, batch: RunsBatch, age: number) =>
	cache.setQueryData(
		batchKey("hashbrown", DEFAULT_SEARCH, batch.batch),
		batch,
		{ updatedAt: Date.now() - age },
	);

const table = () => page.getByRole("table", { name: "Runs, newest first" });
const drawn = () =>
	[...document.querySelectorAll<HTMLElement>("tbody tr[aria-rowindex]")].map(
		(row) => Number(row.getAttribute("aria-rowindex")),
	);
const asked = (requests: URL[], batch: number) =>
	requests.filter((url) => url.searchParams.get("page") === String(batch))
		.length;
const toBottom = () =>
	window.scrollTo({ top: document.body.scrollHeight, behavior: "instant" });
const suspended = () => document.body.textContent?.includes("Suspended");
const wait = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

beforeEach(async () => {
	await page.viewport(1280, 720);
	perPage = screenBatch();
});

afterEach(() => {
	dispose?.();
	document.body.replaceChildren();
	window.scrollTo({ top: 0, behavior: "instant" });
});

// Kills a list read before it arrives, which suspends the page so its own
// placeholders never show, and a skeleton that flashes for data that arrives quickly.
test("a list never fetched draws the page, its skeleton only after a wait", async () => {
	const { api } = fakeApi(() => new Promise<never>(() => {}));
	mount(api, client());

	await expect
		.element(page.getByRole("heading", { level: 1, name: "Runs" }))
		.toBeVisible();
	const skeleton = () =>
		document.querySelector(".reports-skeleton .ui-skeleton") as HTMLElement;
	await expect.poll(skeleton).toBeTruthy();
	expect(getComputedStyle(skeleton()).visibility).toBe("hidden");
	await expect
		.poll(() => getComputedStyle(skeleton()).visibility, { timeout: 2_000 })
		.toBe("visible");
	expect(suspended()).toBe(false);
});

// Kills a failed list that shows nothing, or a Retry that does not ask again.
test("a list the API did not answer shows a banner whose Retry loads it", async () => {
	let fail = true;
	const { api } = fakeApi(() =>
		fail
			? Promise.reject(new ApiError(503, "server", "down"))
			: answered([runFixture(0)], 1),
	);
	mount(api, impatient());

	const banner = page.getByRole("alert");
	await expect.element(banner).toHaveTextContent("Runs did not load");
	fail = false;
	await banner.getByRole("button", { name: "Retry" }).click();

	await expect.element(table()).toBeVisible();
	await expect
		.element(page.getByRole("heading", { level: 1 }))
		.toHaveTextContent("1 run");
	await expect.element(page.getByRole("alert")).not.toBeInTheDocument();
});

// Kills rows that wait for the API when the browser has seen them, a skeleton
// drawn over them, and a header count of the rows rather than the total.
test("a list already seen paints at once while it revalidates", async () => {
	const { api, requests } = fakeApi(() => new Promise<never>(() => {}));
	const cache = client();
	cached(cache, batchOf(1, 1_000), 60_000);
	mount(api, cache);

	await expect.element(table()).toBeVisible();
	await expect
		.element(page.getByRole("heading", { level: 1 }))
		.toHaveTextContent("1000 runs");
	expect(document.querySelector('[aria-busy="true"]')).toBeNull();
	await expect.poll(() => requests.length).toBe(1);
	expect(suspended()).toBe(false);
});

// Kills a page the layout's fallback replaces whenever a batch it shows is
// written again: focus drops and an open dialog stops being modal.
test("a revalidation that answers never takes the page out", async () => {
	const { api } = fakeApi(async () => {
		await wait(50);
		return answered(runs(0, perPage), 1_001);
	});
	const cache = client();
	cached(cache, batchOf(1, 1_000), 60_000);
	const stop = watchFallback();
	mount(api, cache);

	await expect
		.element(page.getByRole("heading", { level: 1 }))
		.toHaveTextContent("1000 runs");
	await expect
		.element(page.getByRole("heading", { level: 1 }))
		.toHaveTextContent("1001 runs");
	await cache.invalidateQueries();
	await wait(200);
	expect(stop()).toBe(false);
});

// Kills a reload that revalidates every batch once scrolled through, and a
// fresh batch asked for again.
test("a stale list revalidates only its first batch, and later batches come from the cache", async () => {
	const { api, requests } = fakeApi(() => new Promise<never>(() => {}));
	const cache = client();
	cached(cache, batchOf(1, 1_000), 60_000);
	cached(cache, batchOf(2, 1_000), 1_000);
	mount(api, cache);

	await expect.element(table()).toBeVisible();
	await expect.poll(() => requests.length).toBe(1);
	expect(asked(requests, 1)).toBe(1);

	toBottom();
	await expect.poll(() => drawn().includes(perPage + 2)).toBe(true);
	await wait(200);
	expect(asked(requests, 2)).toBe(0);
});

// Kills a later batch the API refused that is asked for again and again,
// or skipped for the ones after it, that fails without a word, or whose Retry
// does nothing.
test("a later batch the API refused shows a row whose Retry loads it, asked for once", async () => {
	let refuse = true;
	const { api, requests } = fakeApi((url) => {
		const batch = Number(url.searchParams.get("page"));
		if (batch === 2 && refuse) {
			return Promise.reject(new ApiError(429, "client", "Too many requests"));
		}
		return answered(runs((batch - 1) * perPage, perPage), 2 * perPage);
	});
	mount(api, client());
	await expect.element(table()).toBeVisible();

	toBottom();
	const alert = page.getByRole("alert");
	await expect.element(alert).toHaveTextContent("These runs did not load");
	await wait(500);
	expect(asked(requests, 2)).toBe(1);
	expect(requests).toHaveLength(2);

	refuse = false;
	await alert.getByRole("button", { name: "Retry" }).click();
	toBottom();
	await expect.poll(() => drawn().includes(2 * perPage + 1)).toBe(true);
	expect(asked(requests, 2)).toBe(2);
	await expect.element(page.getByRole("alert")).not.toBeInTheDocument();
});

// Kills a run drawn twice when a new one shifts the batches, and row
// numbers with a gap where it would have been.
test("batches that overlap show each run once, numbered without a gap", async () => {
	const { api } = fakeApi((url) => {
		const batch = Number(url.searchParams.get("page"));
		// A run arrived between the two requests, so the second repeats one.
		const from = batch === 1 ? 0 : perPage - 1;
		return answered(runs(from, perPage), 2 * perPage - 1);
	});
	mount(api, client());
	await expect.element(table()).toBeVisible();

	await expect
		.poll(() => {
			toBottom();
			return drawn().at(-1);
		})
		.toBe(2 * perPage);
	const rows = drawn();
	expect(rows).toEqual(
		Array.from({ length: rows.length }, (_, index) => (rows[0] ?? 0) + index),
	);
	const links = [
		...document.querySelectorAll<HTMLAnchorElement>("tbody a.reports-link"),
	].map((link) => link.getAttribute("href"));
	expect(new Set(links).size).toBe(links.length);
});

// Kills paging that goes on past a batch shorter than asked, which the API
// returns when it drops a run it cannot read.
test("a short batch ends the list", async () => {
	const { api, requests } = fakeApi(() =>
		answered(runs(0, perPage - 1), 1_000),
	);
	mount(api, client());
	await expect.element(table()).toBeVisible();

	toBottom();
	await wait(300);
	toBottom();
	await wait(300);
	expect(requests).toHaveLength(1);
});

// Kills a filter that matches nothing named by its slug.
test("filters that match nothing are named, not slugged", async () => {
	const { api } = fakeApi((url) => {
		if (url.pathname.endsWith("/branches/412-merge")) {
			return { data: { uuid: "b", name: "412/merge", slug: "412-merge" } };
		}
		return answered([], 0);
	});
	mount(api, client(), `${PATH}?branch=412-merge&window=all`);

	await expect
		.element(page.getByRole("heading", { name: "No runs" }))
		.toBeVisible();
	await expect
		.element(page.getByRole("button", { name: "branch 412/merge" }))
		.toBeInTheDocument();
});

// Kills a new search that asks for as many batches as the last one had loaded.
test("a new search starts again from one batch", async () => {
	const { api, requests } = fakeApi((url) => {
		const batch = Number(url.searchParams.get("page"));
		return answered(runs((batch - 1) * perPage, perPage), 10 * perPage);
	});
	mount(api, client());
	await expect.element(table()).toBeVisible();
	await expect
		.poll(() => {
			toBottom();
			return asked(requests, 3);
		})
		.toBe(1);

	window.scrollTo({ top: 0, behavior: "instant" });
	const before = requests.length;
	await page.getByRole("radio", { name: "All" }).click();
	await expect.poll(() => requests.length).toBe(before + 1);
	await wait(300);
	expect(
		requests.slice(before).map((url) => url.searchParams.get("page")),
	).toEqual(["1"]);
});

// Kills a window change that drops the rows already on screen for the
// skeleton, rows kept but not marked busy or not dimmed, and a busy mark that
// stays once the new list answers.
test("a window change keeps the rows on screen, dimmed and busy, until the new list answers", async () => {
	let answer: (value: { data: unknown }) => void = () => {};
	const { api } = fakeApi((url) =>
		url.searchParams.has("start_time")
			? answered(runs(0, perPage), 1_000)
			: new Promise((resolve) => {
					answer = resolve;
				}),
	);
	mount(api, client());
	await expect
		.element(page.getByRole("heading", { level: 1 }))
		.toHaveTextContent("1000 runs");

	await page.getByRole("radio", { name: "All" }).click();
	await expect.element(table()).toHaveAttribute("aria-busy", "true");
	expect(drawn()[0]).toBe(2);
	const body = () => document.querySelector("tbody") as HTMLElement;
	await expect
		.poll(() => Number(getComputedStyle(body()).opacity))
		.toBeLessThan(1);
	expect(document.querySelector(".reports-skeleton")).toBeNull();

	answer(answered([runFixture(7_000)], 1));
	await expect
		.element(page.getByRole("heading", { level: 1 }))
		.toHaveTextContent("1 run");
	await expect.element(table()).not.toHaveAttribute("aria-busy");
	expect(Number(getComputedStyle(body()).opacity)).toBe(1);
	expect(drawn()).toEqual([2]);
});

// Kills a pick that asks the API again for the name its option already showed,
// for the branch or for the testbed.
test("a picked branch or testbed is named from its option, not asked for again", async () => {
	const { api, requests } = fakeApi((url) => {
		const path = url.pathname;
		if (path.endsWith("/branches")) {
			return {
				data: [{ uuid: "b", name: "feature/simd", slug: "feature-simd" }],
			};
		}
		if (path.endsWith("/testbeds")) {
			return { data: [{ uuid: "t", name: "Bare metal", slug: "bare-metal" }] };
		}
		if (path.includes("/branches/") || path.includes("/testbeds/")) {
			return { data: { uuid: "x", name: "asked again", slug: "x" } };
		}
		return answered(runs(0, perPage), 1_000);
	});
	mount(api, client());
	await expect.element(table()).toBeVisible();

	for (const [filter, name] of [
		["branch", "feature/simd"],
		["testbed", "Bare metal"],
	] as const) {
		await page.getByRole("button", { name: `${filter} any` }).click();
		await page.getByRole("menuitemradio", { name }).click();
		await expect
			.element(page.getByRole("button", { name: `${filter} ${name}` }))
			.toBeInTheDocument();
	}
	await expect
		.poll(() => requests.some((url) => url.searchParams.has("testbed")))
		.toBe(true);
	await wait(200);
	expect(
		requests.filter(
			(url) =>
				url.pathname.includes("/branches/") ||
				url.pathname.includes("/testbeds/"),
		),
	).toEqual([]);
});

// Kills a zero state kept from the last search while the next one loads,
// which names the new search as if it had found nothing.
test("a search after nothing was found shows no zero state while it loads", async () => {
	let answer: (value: { data: unknown }) => void = () => {};
	const { api } = fakeApi((url) =>
		url.searchParams.has("start_time")
			? answered([], 0)
			: new Promise((resolve) => {
					answer = resolve;
				}),
	);
	mount(api, client());
	const none = page.getByRole("heading", { name: "No runs", exact: true });
	await expect.element(none).toBeVisible();

	await page.getByRole("button", { name: "Show all time" }).click();
	await wait(100);
	expect(none.elements()).toEqual([]);

	answer(answered([runFixture(0)], 1));
	await expect.element(table()).toBeVisible();
});

// Kills a Runs list wider than a small laptop's screen (a minimum width past
// it), which scrolls sideways where every column fits, and one that cuts off a
// header or three-digit alerts there.
test("at 1024 px the Runs list fits its screen", async () => {
	await page.viewport(1024, 768);
	const { api } = fakeApi(() =>
		answered(
			runs(0, 8).map((run) => ({
				...run,
				testbeds: ["ubuntu-22.04-arm64-large-runner", "macos-latest"],
				testbed_count: 2,
				alerts: { total: 120, active: 64 },
			})),
			8,
		),
	);
	mount(api, client());
	await expect.element(table()).toBeVisible();
	const scroll = table().element().closest(".ui-table-scroll") as HTMLElement;
	expect(scroll.scrollWidth).toBe(scroll.clientWidth);
	for (const header of table().element().querySelectorAll("thead th")) {
		expect(header.scrollWidth, header.textContent ?? "").toBeLessThanOrEqual(
			header.clientWidth,
		);
	}
	const alerts = table().element().querySelector(".reports-alerts")
		?.parentElement as HTMLElement;
	expect(alerts.scrollWidth).toBeLessThanOrEqual(alerts.clientWidth);
});

/** The fixed-length columns, which hold their content whole from 768 px up (rulings 409 and 410). */
const FIXED = ["When", "Head", "Reports", "Lines", "Alerts", "Status"];
const runRows = () => [...document.querySelectorAll("tbody tr[aria-rowindex]")];

// Kills a Runs list that scrolls sideways on a small laptop, and one that cuts
// a likely run's fixed-length cell (its time, head, counts, alerts, or status)
// or a header there; its branch, testbed, adapter, and author take the
// shortfall (ruling 409).
test("at 1024 px a likely run's fixed-length cells and every header are whole", async () => {
	await page.viewport(1024, 768);
	const { api } = fakeApi(() => answered(likelyRuns(Date.now()), 4));
	mount(api, client());
	await expect.element(table()).toBeVisible();
	await document.fonts.ready;
	const scroll = table().element().closest(".ui-table-scroll") as HTMLElement;
	expect(scroll.scrollWidth).toBe(scroll.clientWidth);
	expect(runRows().flatMap((row) => cutCells(row, FIXED))).toEqual([]);
	expect(cutHeaders(table().element())).toEqual([]);
});

// Kills a Runs list column cut below a likely run's text on a desktop, and a
// Status column wider than its longest pill, whose room the text columns need
// (ruling 406).
test("at 1280 px a likely run's every cell is whole, Status no wider than its longest pill", async () => {
	await page.viewport(1280, 768);
	const { api } = fakeApi(() => answered(likelyRuns(Date.now()), 4));
	mount(api, client());
	await expect.element(table()).toBeVisible();
	await document.fonts.ready;
	const pills = [...document.querySelectorAll<HTMLElement>("tbody .rn-st")];
	expect(pills.map((pill) => pill.textContent)).toEqual([
		"Running 59m",
		"Failed 59m 59s",
		"Failed 25h 59m",
	]);
	expect(runRows().flatMap((row) => cutCells(row))).toEqual([]);
	expect(cutHeaders(table().element())).toEqual([]);
	const status = (pills[0] as HTMLElement).closest("td") as HTMLElement;
	const longest = Math.max(
		...pills.map((pill) => pill.getBoundingClientRect().width),
	);
	// Its start padding and a pixel to spare.
	expect(status.getBoundingClientRect().width - longest).toBeLessThanOrEqual(
		12,
	);
});

// Kills a Runs list below a laptop wider or narrower than on a 1024 px page (a
// minimum width other than that layout's), one that cuts a fixed-length cell
// or a header there rather than scroll, and an Alerts column that cuts a
// four-digit total there further than on that page (ruling 410).
test("from 768 to 1023 px the Runs list keeps its 1024 px layout and scrolls sideways", async () => {
	const measure = async (width: number) => {
		await page.viewport(width, 768);
		const { api } = fakeApi(() =>
			answered(
				[
					...likelyRuns(Date.now()),
					likelyRun(4, { alerts: { total: 9999, active: 999 } }),
				],
				5,
			),
		);
		mount(api, client());
		await expect.element(table()).toBeVisible();
		await document.fonts.ready;
		const rows = runRows();
		const four = (rows.at(-1) as Element)
			.querySelector(".reports-alerts")
			?.closest("td") as HTMLElement;
		const scroll = table().element().closest(".ui-table-scroll") as HTMLElement;
		const found = {
			width: table().element().getBoundingClientRect().width,
			cut: [
				...rows.slice(0, -1).flatMap((row) => cutCells(row, FIXED)),
				...cutHeaders(table().element()),
			],
			four: overrun(four),
			scrolls: scroll.scrollWidth > scroll.clientWidth,
		};
		dispose?.();
		dispose = undefined;
		document.body.replaceChildren();
		return found;
	};
	const laptop = await measure(1024);
	expect(laptop.scrolls).toBe(false);
	for (const width of [768, 900]) {
		const below = await measure(width);
		expect(below.width, `at ${width} px`).toBeCloseTo(laptop.width, 0);
		expect(below.cut, `at ${width} px`).toEqual([]);
		expect(below.four, `at ${width} px`).toBeLessThanOrEqual(laptop.four);
		expect(below.scrolls, `at ${width} px`).toBe(true);
	}
});
