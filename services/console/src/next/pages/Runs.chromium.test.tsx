import "@bencherdev/ui/styles.css";
import "../styles/console.css";
import "../reports/reports.css";
import {
	MemoryRouter,
	Route,
	createMemoryHistory,
	useLocation,
} from "@solidjs/router";
import { type QueryClient, QueryClientProvider } from "@tanstack/solid-query";
import { Suspense } from "solid-js";
import { render } from "solid-js/web";
import { afterEach, beforeEach, expect, test } from "vitest";
import { page, userEvent } from "vitest/browser";
import type { JsonConsoleRunSummary } from "../../types/bencher";
import { type Api, ApiError } from "../api";
import { createQueryClient } from "../cache";
import { ProjectContext } from "../project";
import { DEFAULT_SEARCH } from "../reports/search";
import { fakeApi, watchFallback } from "../reports/testing";
import { screenBatch } from "../list/rows";
import { screenBatch as reportScreenBatch } from "../report/query";
import { type RunsBatch, batchKey } from "../runs/query";
import { runReportsQuery } from "../runs/status";
import { bootstrapOf } from "../settings/testing";
import { nameFloor } from "../ui/testing";
import {
	cutCells,
	cutHeaders,
	likelyRun,
	likelyRuns,
	runFixture,
	runPageFixture,
	runReportFixture,
} from "../runs/testing";
import Runs from "./Runs";

// Relative to the router base, as the app routes it.
const PATH = "/hashbrown/runs";

let dispose: (() => void) | undefined;
let mounted: ReturnType<typeof createMemoryHistory> | undefined;

/** The page under a Suspense boundary, as it sits under the layout's, at the project root and under Runs. */
const mount = (api: Api, client: QueryClient, path = PATH) => {
	const history = createMemoryHistory();
	mounted = history;
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
					<Route path="/:project" component={Runs} />
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
	window.history.replaceState(null, "");
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
		.toHaveTextContent("1,000 runs");
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
		.toHaveTextContent("1,000 runs");
	await expect
		.element(page.getByRole("heading", { level: 1 }))
		.toHaveTextContent("1,001 runs");
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
		...document.querySelectorAll<HTMLAnchorElement>("tbody .reports-hash a"),
	].map((link) => link.getAttribute("href"));
	expect(links.length).toBeGreaterThan(0);
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
		.toHaveTextContent("1,000 runs");

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
				data: {
					branches: [{ uuid: "b", name: "feature/simd", slug: "feature-simd" }],
				},
			};
		}
		if (path.endsWith("/testbeds")) {
			return {
				data: {
					testbeds: [{ uuid: "t", name: "Bare metal", slug: "bare-metal" }],
				},
			};
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
	mount(api, client(), `${PATH}?window=1w`);
	const none = page.getByRole("heading", { name: "No runs", exact: true });
	await expect.element(none).toBeVisible();

	await page.getByRole("button", { name: "Show all time" }).click();
	await wait(100);
	expect(none.elements()).toEqual([]);

	answer(answered([runFixture(0)], 1));
	await expect.element(table()).toBeVisible();
});

// Kills a Runs list wider than a laptop's screen (a minimum
// width past it), which scrolls sideways where every column fits, and one that
// cuts off a header or three-digit alerts there.
test("at 1280 px the Runs list fits its screen", async () => {
	await page.viewport(1280, 768);
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

/** The fixed-length columns, which hold their content whole wherever the list does not fold (rulings 409 and 410). */
const FIXED = ["When", "Head", "Reports", "Lines", "Alerts", "Status"];
const runRows = () => [...document.querySelectorAll("tbody tr[aria-rowindex]")];

// Kills a Runs list that scrolls sideways on a laptop, and one that
// cuts a likely run's fixed-length cell (its time, head, counts, alerts, or
// status) or a header there; its branch, testbed, adapter, and author take the
// shortfall (ruling 409).
test("at 1280 px a likely run's fixed-length cells and every header are whole", async () => {
	await page.viewport(1280, 768);
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

// Kills a Runs list on a tablet or a small laptop that keeps its columns,
// whose Branch falls under 16 characters there, or scrolls sideways (ruling
// 410, replaced by the name floor).
test("on a tablet and a small laptop the Runs list folds, never scrolling sideways", async () => {
	for (const width of [768, 1024]) {
		await page.viewport(width, 768);
		const { api } = fakeApi(() => answered(likelyRuns(Date.now()), 4));
		mount(api, client());
		await expect.element(table()).toBeVisible();
		expect(nameFloor(table().element(), "Branch"), `at ${width} px`).toEqual({
			name: "folded",
			sideways: false,
		});
		dispose?.();
		dispose = undefined;
		document.body.replaceChildren();
	}
});

const card = () => page.getByRole("region", { name: "Send the first report" });
const BOOTSTRAP = "/v0/console/projects/hashbrown";

/** Watch for the zero state or a zero count; the returned call says whether either was ever drawn. */
const watchNoRuns = () => {
	let drawn = false;
	const watcher = new MutationObserver(() => {
		drawn ||=
			[...document.querySelectorAll("h2")].some(
				(heading) => heading.textContent === "No runs",
			) ||
			[...document.querySelectorAll("h1")].some((heading) =>
				heading.textContent?.includes("0 runs"),
			);
	});
	watcher.observe(document.body, { childList: true, subtree: true });
	return () => {
		watcher.disconnect();
		return drawn;
	};
};

// Kills a project that never ran drawn as an empty list, or as "No runs" while
// Runs asks whether it ever ran, a card that never notices the first run, and
// a card that gives way to anything but the list with that run on top.
test("a project that never ran draws the setup card at the root, which gives way to the list once a run lands", async () => {
	let ran = false;
	const { api } = fakeApi((url) => {
		if (url.pathname === BOOTSTRAP) {
			return { data: bootstrapOf() };
		}
		return ran ? answered([runFixture(0)], 1) : answered([], 0);
	});
	const noRuns = watchNoRuns();
	mount(api, client(), "/hashbrown");
	await expect.element(card()).toBeVisible();
	expect(table().query()).toBeNull();

	ran = true;
	await expect.element(table(), { timeout: 8_000 }).toBeVisible();
	expect(card().query()).toBeNull();
	expect(drawn()).toEqual([2]);
	expect(
		table().getByRole("link").first().element().getAttribute("href"),
	).toContain(runFixture(0).uuid);
	expect(noRuns()).toBe(false);
});

// Kills the setup card drawn for a project whose runs all predate the
// window, and a Show all time that asks again for what the check already read.
test("a project whose runs predate the window says No runs, and Show all time lists them from the check", async () => {
	const { api, requests } = fakeApi((url) =>
		url.searchParams.has("start_time")
			? answered([], 0)
			: answered([runFixture(0)], 1),
	);
	mount(api, client(), "/hashbrown");
	await expect
		.element(page.getByRole("heading", { name: "No runs", exact: true }))
		.toBeVisible();
	expect(card().query()).toBeNull();
	const asked = requests.length;
	await page.getByRole("button", { name: "Show all time" }).click();
	await expect.element(table()).toBeVisible();
	expect(requests.length).toBe(asked);
});

// Kills a page held on its skeleton for good when the check for whether the
// project ever ran fails.
test("a failed check for whether the project ever ran says No runs", async () => {
	const { api } = fakeApi((url) => {
		if (!url.searchParams.has("start_time")) {
			return Promise.reject(new ApiError(503, "server", "down"));
		}
		return answered([], 0);
	});
	mount(api, impatient(), "/hashbrown");
	await expect
		.element(page.getByRole("heading", { name: "No runs", exact: true }))
		.toBeVisible();
	expect(card().query()).toBeNull();
});

// Kills a filtered search that matches nothing drawn as the setup card.
test("a search that matches nothing in a project that never ran says No runs", async () => {
	const { api, requests } = fakeApi(() => answered([], 0));
	mount(api, client(), "/hashbrown?alerts=active");
	await expect
		.element(page.getByRole("heading", { name: "No runs", exact: true }))
		.toBeVisible();
	expect(card().query()).toBeNull();
	expect(requests.length).toBe(1);
});

// Kills a search at the project root that moves the reader to the Runs path,
// which mounts the page again and drops the rows on screen.
test("a search at the project root stays at the root and keeps its rows on screen", async () => {
	let answer: (value: { data: unknown }) => void = () => {};
	const { api } = fakeApi((url) =>
		url.searchParams.has("start_time")
			? answered(runs(0, perPage), 1_000)
			: new Promise((resolve) => {
					answer = resolve;
				}),
	);
	mount(api, client(), "/hashbrown");
	await expect.element(table()).toBeVisible();
	const shown = table().element();

	await page.getByRole("radio", { name: "All" }).click();
	await expect.poll(() => mounted?.get()).toBe("/hashbrown?window=all");
	await expect.element(table()).toHaveAttribute("aria-busy", "true");
	expect(table().element()).toBe(shown);
	answer(answered([runFixture(7_000)], 1));
	await expect
		.element(page.getByRole("heading", { level: 1 }))
		.toHaveTextContent("1 run");
});

/** A run's first reports as the run endpoint answers them: `count` of `total`. */
const firstReports = (
	run: JsonConsoleRunSummary,
	count: number,
	total = count,
) =>
	runPageFixture(
		Array.from({ length: count }, (_, index) => runReportFixture(index)),
		{ uuid: run.uuid, total },
	);

/** An API that lists `listed` and answers each run's reports with `count` of them. */
const runsApi = (listed: JsonConsoleRunSummary[], count = 2) =>
	fakeApi((url) => {
		const run = listed.find((each) =>
			url.pathname.endsWith(`/runs/${each.uuid}`),
		);
		return run
			? { data: firstReports(run, count) }
			: answered(listed, listed.length);
	});

const toggles = () => [...document.querySelectorAll<HTMLElement>(".rn-toggle")];
const expanded = () =>
	toggles().map((toggle) => toggle.getAttribute("aria-expanded"));
const reportsAsked = (requests: URL[], run: JsonConsoleRunSummary) =>
	requests.filter((url) => url.pathname.endsWith(`/runs/${run.uuid}`));

// Kills a run whose reports are asked for only once clicked, asked for in
// more than its first eight, or asked for again on the click.
test("a run asks for its first eight reports as the pointer arrives, and opening it asks nothing more", async () => {
	const run = runFixture(0, { reports: 2 });
	const { api, requests } = runsApi([run, runFixture(1)]);
	mount(api, client());
	const toggle = table().getByRole("button", {
		name: /^main, run 9c1f2e4, 2 reports, /,
	});
	await toggle.hover();
	await expect.poll(() => reportsAsked(requests, run).length).toBe(1);
	expect(reportsAsked(requests, run)[0]?.searchParams.get("per_page")).toBe(
		"8",
	);
	await toggle.click();
	await expect
		.element(table().getByRole("link", { name: /^ubuntu-latest, report 2, / }))
		.toBeVisible();
	await wait(100);
	expect(reportsAsked(requests, run)).toHaveLength(1);
});

// Kills a Runs list that reads an opened run's reports from anything but the
// query blank Explore reads them with, so a run opened there waits here.
test("a run blank Explore already opened draws its reports from what Explore read", async () => {
	const run = runFixture(0, { reports: 2 });
	const { api, requests } = fakeApi((url) =>
		url.pathname.endsWith(`/runs/${run.uuid}`)
			? new Promise<never>(() => {})
			: answered([run], 1),
	);
	const cache = client();
	cache.setQueryData(
		runReportsQuery(api, "hashbrown", run.uuid).queryKey,
		firstReports(run, 2),
	);
	mount(api, cache);
	await table()
		.getByRole("button", { name: /^main, run 9c1f2e4, / })
		.click();
	await expect
		.element(table().getByRole("link", { name: /^ubuntu-latest, report 2, / }))
		.toBeVisible();
	expect(reportsAsked(requests, run)).toEqual([]);
});

/** Watch `root` for a toggle drawn closed and then opened; the returned call stops and says whether one was. */
const watchReopen = (root: Node) => {
	let reopened = false;
	const watcher = new MutationObserver((records) => {
		reopened ||= records.some((record) => record.oldValue === "false");
	});
	watcher.observe(root, {
		subtree: true,
		attributes: true,
		attributeOldValue: true,
		attributeFilter: ["aria-expanded"],
	});
	return () => {
		watcher.disconnect();
		return reopened;
	};
};

// Kills opened runs kept in the URL, forgotten when the page goes and comes
// back with Back, restored after the first draw (so the rows below jump and
// the browser's scroll lands elsewhere), or kept for a new visit.
test("the opened runs come back with Back, drawn open at once, and a new visit opens none", async () => {
	const runs = [runFixture(0, { reports: 2 }), runFixture(1, { reports: 2 })];
	const { api } = runsApi(runs);
	const cache = client();
	mount(api, cache);
	await expect.element(table()).toBeVisible();
	await userEvent.click(toggles()[0] as HTMLElement);
	await expect.poll(expanded).toEqual(["true", "false"]);
	expect(mounted?.get()).toBe(PATH);

	// A move to another page: a new history entry, and this page goes.
	window.history.pushState(null, "");
	dispose?.();
	document.body.replaceChildren();
	const popped = new Promise((resolve) =>
		window.addEventListener("popstate", resolve, { once: true }),
	);
	window.history.back();
	await popped;
	const reopened = watchReopen(document.body);
	mount(api, cache);
	await expect.poll(expanded).toEqual(["true", "false"]);
	expect(drawn()).toEqual([2, 3, 4, 5]);
	expect(reopened()).toBe(false);

	window.history.pushState(null, "");
	dispose?.();
	document.body.replaceChildren();
	mount(api, cache);
	await expect.poll(expanded).toEqual(["false", "false"]);
	expect(drawn()).toEqual([2, 3]);
});

// Kills a new search that closes an opened run it still lists, or keeps open
// one it no longer lists, so it opens again once a later search lists it.
test("a new search keeps an opened run it lists open and closes the rest", async () => {
	const [kept, dropped, other] = [
		runFixture(0, { reports: 1 }),
		runFixture(1, { reports: 1 }),
		runFixture(2, { reports: 1 }),
	];
	const { api } = fakeApi((url) => {
		const run = [kept, dropped, other].find((each) =>
			url.pathname.endsWith(`/runs/${each.uuid}`),
		);
		if (run) {
			return { data: firstReports(run, 1) };
		}
		return url.searchParams.has("start_time")
			? answered([kept, dropped], 2)
			: answered([kept, other], 2);
	});
	mount(api, client());
	await expect.element(table()).toBeVisible();
	toggles()[0]?.click();
	toggles()[1]?.click();
	await expect.poll(expanded).toEqual(["true", "true"]);

	await page.getByRole("radio", { name: "All" }).click();
	await expect
		.poll(() => table().element().getAttribute("aria-busy"))
		.toBe(null);
	await expect.poll(expanded).toEqual(["true", "false"]);
	await page.getByRole("radio", { name: "4w" }).click();
	await expect.poll(expanded).toEqual(["true", "false"]);
	expect(drawn()).toEqual([2, 3, 4]);
});

// Kills a report of an opened run whose page is not asked for before the
// click, or is asked for in another view or batch size than its page asks.
test("hovering an opened run's report starts its page's first batch in the default view", async () => {
	const run = runFixture(0, { reports: 1 });
	const { api, requests } = fakeApi((url) => {
		if (url.pathname.includes("/reports/")) {
			return { data: {} };
		}
		return url.pathname.endsWith(`/runs/${run.uuid}`)
			? { data: firstReports(run, 1) }
			: answered([run], 1);
	});
	mount(api, client());
	await table()
		.getByRole("button", { name: /^main, run / })
		.click();
	const report = table().getByRole("link", {
		name: /^ubuntu-latest, report 1, /,
	});
	await report.hover();
	const asked = () =>
		requests.filter((url) => url.pathname.includes("/reports/"));
	await expect.poll(() => asked().length).toBe(1);
	expect(asked()[0]?.pathname).toBe(
		`/v0/console/projects/hashbrown/reports/${firstReports(run, 1).reports[0]?.uuid}`,
	);
	expect(asked()[0]?.searchParams.toString()).toBe(
		`window=28&page=1&per_page=${reportScreenBatch()}`,
	);
});

// Kills an opened run whose failed read never says so, or whose Retry does
// not ask again.
test("an opened run whose reports did not load shows a row whose Retry loads them", async () => {
	const run = runFixture(0, { reports: 2 });
	let fail = true;
	const { api } = fakeApi((url) => {
		if (url.pathname.endsWith(`/runs/${run.uuid}`)) {
			return fail
				? Promise.reject(new ApiError(503, "server", "down"))
				: { data: firstReports(run, 2) };
		}
		return answered([run], 1);
	});
	mount(api, impatient());
	await table()
		.getByRole("button", { name: /^main, run / })
		.click();
	const alert = table().getByRole("alert");
	await expect.element(alert).toHaveTextContent("These reports did not load");
	fail = false;
	await alert.getByRole("button", { name: "Retry" }).click();
	await expect
		.element(table().getByRole("link", { name: /^ubuntu-latest, report 2, / }))
		.toBeVisible();
	await expect.element(table().getByRole("alert")).not.toBeInTheDocument();
});

// Kills a next batch counted by rows rather than runs, which comes while
// opened runs' reports still fill the screen and many runs lie below, and one
// that never comes, or comes twice, once the reader scrolls to the end.
test("opened runs neither hurry, stall, nor double the next batch", async () => {
	const { api, requests } = fakeApi((url) => {
		if (url.pathname.includes("/runs/")) {
			return {
				data: runPageFixture(
					Array.from({ length: 8 }, (_, index) => runReportFixture(index)),
				),
			};
		}
		const batch = Number(url.searchParams.get("page"));
		return answered(
			runs((batch - 1) * perPage, perPage).map((run) => ({
				...run,
				reports: 8,
			})),
			2 * perPage,
		);
	});
	mount(api, client());
	await expect.element(table()).toBeVisible();
	toggles()[0]?.click();
	toggles()[1]?.click();
	await expect
		.poll(() => expanded().slice(0, 3))
		.toEqual(["true", "true", "false"]);
	await expect.poll(() => drawn().includes(19)).toBe(true);

	// Rows past the first batch's count, but runs well inside it.
	const body = document.querySelector("tbody") as HTMLElement;
	window.scrollTo({
		top: body.getBoundingClientRect().top + window.scrollY + 13 * 40,
		behavior: "instant",
	});
	await wait(300);
	expect(asked(requests, 2)).toBe(0);

	await expect
		.poll(() => {
			toBottom();
			return drawn().at(-1);
		})
		.toBe(2 * perPage + 16 + 1);
	await wait(300);
	expect(asked(requests, 2)).toBe(1);
});

// Kills a refetch whose answer, unchanged, draws an opened run's reports again
// and takes the focus from them.
test("an opened run's reports asked again, unchanged, keep their rows and the focus", async () => {
	const run = runFixture(0, { reports: 2 });
	const { api, requests } = runsApi([run]);
	const cache = client();
	mount(api, cache);
	await table()
		.getByRole("button", { name: /^main, run / })
		.click();
	const link = table().getByRole("link", {
		name: /^ubuntu-latest, report 2, /,
	});
	await expect.element(link).toBeVisible();
	const shown = link.element() as HTMLElement;
	shown.focus();
	const before = reportsAsked(requests, run).length;
	await cache.invalidateQueries();
	await expect.poll(() => reportsAsked(requests, run).length).toBe(before + 1);
	await wait(100);
	expect(link.element()).toBe(shown);
	expect(document.activeElement).toBe(shown);
});

// Kills an opened run's likely report cut in a fixed-length cell (its time,
// counts, alerts, or status) on a laptop or wider.
test("from 1280 px an opened likely run's reports keep their fixed-length cells whole", async () => {
	for (const width of [1280, 1440]) {
		await page.viewport(width, 768);
		const run = likelyRun(0, { reports: 2 });
		const { api } = fakeApi((url) =>
			url.pathname.endsWith(`/runs/${run.uuid}`)
				? {
						data: runPageFixture(
							[0, 1].map((index) =>
								runReportFixture(index, {
									adapter: run.adapters[0],
									alerts: { total: 456, active: 123 },
									start_time: Date.now() - 22 * 60 * 60_000 - 23 * 60_000,
								}),
							),
						),
					}
				: answered([run], 1),
		);
		mount(api, client());
		await table()
			.getByRole("button", { name: /^feature-simd, / })
			.click();
		await expect.poll(() => drawn().length).toBe(3);
		await document.fonts.ready;
		expect(runRows().flatMap((row) => cutCells(row, FIXED))).toEqual([]);
		dispose?.();
		dispose = undefined;
		document.body.replaceChildren();
		window.history.replaceState(null, "");
	}
});

// Kills a history entry whose other state, the router's place in history
// among it, is lost when the page keeps the opened runs on it.
test("the opened runs join the history entry's state and keep what else it holds", async () => {
	const run = runFixture(0, { reports: 2 });
	const { api } = runsApi([run]);
	window.history.replaceState({ _depth: 7, other: "kept" }, "");
	mount(api, client());
	await expect.element(table()).toBeVisible();
	toggles()[0]?.click();
	await expect.poll(expanded).toEqual(["true"]);
	expect(window.history.state).toEqual({
		_depth: 7,
		other: "kept",
		runs: [run.uuid],
	});
});

/** Two projects' lists, each listing one run: `mine` in hashbrown, `theirs` in croissant. */
const twoProjects = () => {
	const [mine, theirs] = [
		runFixture(0, { reports: 1 }),
		{
			...runFixture(1, { reports: 1 }),
			uuid: "11111111-0000-4000-8000-000000000000",
		},
	];
	const { api, requests } = fakeApi((url) => {
		const project = url.pathname.includes("/croissant/") ? theirs : mine;
		const run = [mine, theirs].find((each) =>
			url.pathname.endsWith(`/runs/${each.uuid}`),
		);
		if (run) {
			return run === project
				? { data: firstReports(run, 1) }
				: Promise.reject(new ApiError(404, "not_found", "Not found"));
		}
		return answered([project], 1);
	});
	return { mine, theirs, api, requests };
};

/** The page at hashbrown's root, its project read from the path, as a move to another project's root keeps the page. */
const mountProjects = (api: Api) => {
	const history = createMemoryHistory();
	history.set({ value: "/hashbrown" });
	const root = document.createElement("div");
	root.className = "console";
	document.body.append(root);
	dispose = render(
		() => (
			<QueryClientProvider client={client()}>
				<MemoryRouter
					history={history}
					root={(props) => {
						const location = useLocation();
						return (
							<ProjectContext.Provider
								value={{
									api,
									slug: () => location.pathname.split("/")[1] ?? "",
								}}
							>
								<Suspense>{props.children}</Suspense>
							</ProjectContext.Provider>
						);
					}}
				>
					<Route path="/:project" component={Runs} />
				</MemoryRouter>
			</QueryClientProvider>
		),
		root,
	);
	return history;
};

/** Move the window's history and the router's one step together, as the browser's Back or Forward does. */
const step = async (
	history: ReturnType<typeof createMemoryHistory>,
	delta: -1 | 1,
) => {
	const popped = new Promise((resolve) =>
		window.addEventListener("popstate", resolve, { once: true }),
	);
	window.history.go(delta);
	await popped;
	history.go(delta);
};

// Kills a list that, once the reader moves to another project's, stops
// keeping the runs opened there, or keeps the first project's opened runs, and
// one that on the way back does not open the runs that project's entry kept.
test("another project's list keeps its own opened runs", async () => {
	const { mine, theirs, api } = twoProjects();
	const history = mountProjects(api);
	await expect.element(table()).toBeVisible();
	toggles()[0]?.click();
	await expect.poll(() => window.history.state?.runs).toEqual([mine.uuid]);

	// The browser's new entry, as the router pushes it once the move ends.
	window.history.pushState(null, "");
	history.set({ value: "/croissant" });
	await expect
		.poll(() => document.querySelector(".reports-hash a")?.getAttribute("href"))
		.toContain(theirs.uuid);
	await expect.poll(() => window.history.state?.runs).toEqual([]);
	expect(expanded()).toEqual(["false"]);
	toggles()[0]?.click();
	await expect.poll(() => window.history.state?.runs).toEqual([theirs.uuid]);

	await step(history, -1);
	await expect.poll(() => window.history.state?.runs).toEqual([mine.uuid]);
	await expect.poll(expanded).toEqual(["true"]);
	expect(
		table()
			.getByRole("link", { name: /^ubuntu-latest, report 1, / })
			.elements(),
	).toHaveLength(1);
});

// Kills an opened run read in the project the list moves to, before the
// move reaches that project's entry, which asks for a run the project does
// not have, on the way there, Back, and Forward.
test("a move to another project's list, Back, and Forward ask for no run in a project it is not in", async () => {
	const { mine, theirs, api, requests } = twoProjects();
	const history = mountProjects(api);
	await expect.element(table()).toBeVisible();
	toggles()[0]?.click();
	await expect.poll(expanded).toEqual(["true"]);

	window.history.pushState(null, "");
	history.set({ value: "/croissant" });
	await expect
		.poll(() => document.querySelector(".reports-hash a")?.getAttribute("href"))
		.toContain(theirs.uuid);
	toggles()[0]?.click();
	await expect.poll(expanded).toEqual(["true"]);
	await step(history, -1);
	await expect
		.poll(() => document.querySelector(".reports-hash a")?.getAttribute("href"))
		.toContain(mine.uuid);
	await expect.poll(expanded).toEqual(["true"]);
	await step(history, 1);
	await expect
		.poll(() => document.querySelector(".reports-hash a")?.getAttribute("href"))
		.toContain(theirs.uuid);
	await expect.poll(expanded).toEqual(["true"]);

	const read = (project: string, run: string) =>
		requests.filter(
			(url) => url.pathname === `/v0/console/projects/${project}/runs/${run}`,
		).length;
	expect(read("hashbrown", mine.uuid)).toBeGreaterThan(0);
	expect(read("croissant", theirs.uuid)).toBeGreaterThan(0);
	expect([
		read("croissant", mine.uuid),
		read("hashbrown", theirs.uuid),
	]).toEqual([0, 0]);
});
