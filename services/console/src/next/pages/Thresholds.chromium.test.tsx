import "@bencherdev/ui/styles.css";
import "../styles/console.css";
import "../reports/reports.css";
import "../thresholds/thresholds.css";
import { MemoryRouter, Route, createMemoryHistory } from "@solidjs/router";
import { type QueryClient, QueryClientProvider } from "@tanstack/solid-query";
import { Suspense } from "solid-js";
import { render } from "solid-js/web";
import { afterEach, beforeEach, expect, test } from "vitest";
import { page, userEvent } from "vitest/browser";
import type { JsonConsoleThresholdRow, ModelTest } from "../../types/bencher";
import { type Api, ApiError } from "../api";
import { createQueryClient } from "../cache";
import { ProjectContext } from "../project";
import { fakeApi } from "../reports/testing";
import { screenBatch } from "../thresholds/query";
import { thresholdsFixture } from "../thresholds/testing";
import Thresholds from "./Thresholds";

// Relative to the router base, as the app routes it.
const PATH = "/hashbrown/thresholds";
const MAIN = "7b5d3f39-ec2c-4b46-9c55-1d0b6c2f5a10";
const LEGACY = "0c0bd5b2-6f0e-4d8e-9a43-2d6f8b1e7c55";
const TESTBED = "3f1d2c4b-5a69-4e8f-8b7c-6d5e4f3a2b1c";
const MEASURE = "e9c1b0c4-6a5e-4f5f-8d1c-3b2a19f0a7d2";

let dispose: (() => void) | undefined;

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
					<Route path="/:project/thresholds" component={Thresholds} />
				</MemoryRouter>
			</QueryClientProvider>
		),
		root,
	);
	return history;
};

const client = () => createQueryClient(() => {});

let perPage = 0;

/** A batch of thresholds, `count` of them from the `from`th, out of `total`. */
const batch = (from: number, count: number, total: number) => ({
	data: {
		...thresholdsFixture(
			Array.from(
				{ length: count },
				(_, index): Partial<JsonConsoleThresholdRow> => ({
					uuid: `00000000-0000-4000-8000-${String(from + index).padStart(12, "0")}`,
				}),
			),
		),
		total,
	},
});

const table = () =>
	page.getByRole("table", { name: /thresholds, with the alerts each raised/i });
const drawn = () =>
	[...document.querySelectorAll<HTMLElement>("tbody tr[aria-rowindex]")].map(
		(row) => Number(row.getAttribute("aria-rowindex")),
	);
const asked = (requests: URL[], page: number) =>
	requests.filter(
		(url) =>
			url.pathname.endsWith("/console/thresholds") &&
			url.searchParams.get("page") === String(page),
	).length;
const toBottom = () =>
	window.scrollTo({ top: document.body.scrollHeight, behavior: "instant" });
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

// Kills a row read against the wrong table entry, a branch's start point or
// archive left out, and a model or filter drawn as anything but its text.
test("a row reads its threshold from the batch's tables", async () => {
	const { api } = fakeApi(() => ({
		data: thresholdsFixture([
			{},
			{
				branch: 1,
				measure: 1,
				metric: "p99",
				parameters: [{ n: 1 }, { n: 2 }],
				model: { test: "percentage" as ModelTest, lower_boundary: 0.1 },
				raised: 0,
				active: 0,
			},
		]),
	}));
	mount(api, client());

	await expect
		.element(page.getByRole("heading", { level: 1 }))
		.toHaveTextContent("2 thresholds");
	const row = table().getByRole("row").nth(2);
	await expect
		.element(row.getByRole("cell").nth(0))
		.toHaveTextContent(/^412\/merge from main · archived Sep 8$/);
	await expect
		.element(row.getByRole("cell").nth(2))
		.toHaveTextContent("Throughput");
	await expect.element(row.getByRole("cell").nth(3)).toHaveTextContent("p99");
	await expect
		.element(row.getByRole("cell").nth(4))
		.toHaveTextContent("n=1 or n=2");
	await expect
		.element(row.getByRole("cell").nth(5))
		.toHaveTextContent("percentage · lower_boundary 0.1");
	await expect
		.element(row.getByRole("cell").nth(6))
		.toHaveTextContent("0 raised");
	await expect
		.element(
			row.getByRole("link", {
				name: "Threshold on 412/merge, ubuntu-latest, Throughput, p99, filtered to 2 parameter sets",
			}),
		)
		.toHaveAttribute(
			"href",
			"/next/console/projects/hashbrown/thresholds/00000000-0000-4000-8000-000000000002",
		);
});

// Kills a refine that drops the rows on screen for a skeleton, or leaves them
// unmarked while the new rows load, and an archived list named as the active one.
test("a new search keeps the rows on screen, marked busy, until it answers", async () => {
	let hold: (() => void) | undefined;
	const { api } = fakeApi(async (url) => {
		if (url.searchParams.get("archived") === "true") {
			await new Promise<void>((resolve) => {
				hold = resolve;
			});
			return batch(100, 1, 1);
		}
		return batch(0, 3, 3);
	});
	mount(api, client());
	await expect
		.element(page.getByRole("heading", { level: 1 }))
		.toHaveTextContent("3 thresholds");

	await page.getByRole("radio", { name: "Archived" }).click();
	await expect.element(table()).toHaveAttribute("aria-busy", "true");
	expect(drawn()).toEqual([2, 3, 4]);
	expect(document.querySelector(".th-skeleton")).toBeNull();

	hold?.();
	await expect
		.element(page.getByRole("heading", { level: 1 }))
		.toHaveTextContent("1 archived threshold");
	await expect.element(table()).not.toHaveAttribute("aria-busy");
	await expect
		.element(
			page.getByRole("table", {
				name: /^Archived thresholds, with the alerts each raised /,
			}),
		)
		.toBeVisible();
});

// Kills a list that never asks past its first batch, asks again for one it
// has, or draws every row it holds rather than those in view.
test("scrolling to the end loads each next batch once, and only rows in view are drawn", async () => {
	const total = 3 * perPage;
	const { api, requests } = fakeApi((url) => {
		const at = Number(url.searchParams.get("page"));
		return batch((at - 1) * perPage, perPage, total);
	});
	mount(api, client());
	await expect.element(table()).toBeVisible();
	expect(drawn().length).toBeLessThan(perPage);

	await expect
		.poll(() => {
			toBottom();
			return drawn().at(-1);
		})
		.toBe(total + 1);
	await wait(200);
	expect([asked(requests, 1), asked(requests, 2), asked(requests, 3)]).toEqual([
		1, 1, 1,
	]);
	expect(asked(requests, 4)).toBe(0);
	expect(drawn().length).toBeLessThan(2 * perPage);
});

// Kills a filter chip that shows a UUID, and a name asked for after the list rather than beside it.
test("a filter in the link names its chip from a request beside the list's", async () => {
	let answer: (() => void) | undefined;
	const { api, requests } = fakeApi(async (url) => {
		if (url.pathname.endsWith(`/branches/${MAIN}`)) {
			return { data: { uuid: MAIN, name: "main", slug: "main" } };
		}
		await new Promise<void>((resolve) => {
			answer = resolve;
		});
		return batch(0, 1, 1);
	});
	mount(api, client(), `${PATH}?branch=${MAIN}`);

	await expect
		.element(page.getByRole("button", { name: "Filter by branch, main" }))
		.toBeVisible();
	expect(requests.map((url) => url.pathname)).toContain(
		"/v0/projects/hashbrown/console/thresholds",
	);
	expect(
		requests
			.find((url) => url.pathname.endsWith("/console/thresholds"))
			?.searchParams.get("branch"),
	).toBe(MAIN);
	answer?.();
});

// Kills a later batch the API refused that fails without a word, or whose
// Retry asks for nothing.
test("a later batch the API refused offers Retry, which loads it", async () => {
	let refuse = true;
	const { api, requests } = fakeApi((url) => {
		const at = Number(url.searchParams.get("page"));
		if (at === 2 && refuse) {
			return Promise.reject(new ApiError(429, "client", "Too many requests"));
		}
		return batch((at - 1) * perPage, perPage, 2 * perPage);
	});
	mount(api, client());
	await expect.element(table()).toBeVisible();

	const failed = table().getByRole("alert");
	await expect
		.poll(
			() => {
				toBottom();
				return failed.query() !== null;
			},
			{ timeout: 5_000 },
		)
		.toBe(true);
	await wait(300);
	expect(asked(requests, 2)).toBe(1);

	refuse = false;
	await failed.getByRole("button", { name: "Retry" }).click();
	await expect
		.poll(
			() => {
				toBottom();
				return drawn().at(-1);
			},
			{ timeout: 5_000 },
		)
		.toBe(2 * perPage + 1);
	expect(asked(requests, 2)).toBe(2);
});

// Kills a new search that asks for as many batches as the last one had loaded.
test("a new search starts again from one batch", async () => {
	const { api, requests } = fakeApi((url) => {
		const at = Number(url.searchParams.get("page"));
		return batch((at - 1) * perPage, perPage, 10 * perPage);
	});
	mount(api, client());
	await expect.element(table()).toBeVisible();
	await expect
		.poll(
			() => {
				toBottom();
				return asked(requests, 3);
			},
			{ timeout: 5_000 },
		)
		.toBe(1);

	window.scrollTo({ top: 0, behavior: "instant" });
	const before = requests.length;
	await page.getByRole("radio", { name: "Archived" }).click();
	await expect.poll(() => requests.length).toBeGreaterThan(before);
	await wait(300);
	expect(
		requests.slice(before).map((url) => url.searchParams.get("page")),
	).toEqual(["1"]);
});

// Kills Clear filters that also resets the status or the window.
test("Clear filters keeps the status and the window", async () => {
	const { api } = fakeApi((url) =>
		url.pathname.endsWith(`/branches/${MAIN}`)
			? { data: { uuid: MAIN, name: "main", slug: "main" } }
			: { data: thresholdsFixture([]) },
	);
	const history = mount(
		api,
		client(),
		`${PATH}?status=archived&branch=${MAIN}&window=1w`,
	);

	await page.getByRole("button", { name: "Clear filters" }).click();
	await expect
		.poll(() => history.get())
		.toBe(`${PATH}?status=archived&window=1w`);
});

// Kills a pick that asks the API again for the name its option already showed,
// for the branch, the testbed, or the measure.
test("a picked branch, testbed, or measure is named from its option, not asked for again", async () => {
	const named = /\/(branches|testbeds|measures)\/./;
	const { api, requests } = fakeApi((url) => {
		const path = url.pathname;
		if (path.endsWith("/branches")) {
			return {
				data: [{ uuid: MAIN, name: "feature/simd", slug: "feature-simd" }],
			};
		}
		if (path.endsWith("/testbeds")) {
			return {
				data: [{ uuid: TESTBED, name: "Bare metal", slug: "bare-metal" }],
			};
		}
		if (path.endsWith("/measures")) {
			return {
				data: [{ uuid: MEASURE, name: "Throughput", slug: "throughput" }],
			};
		}
		if (named.test(path)) {
			return { data: { uuid: "x", name: "asked again", slug: "x" } };
		}
		return batch(0, 1, 1);
	});
	mount(api, client());
	await expect.element(table()).toBeVisible();

	for (const [filter, name] of [
		["branch", "feature/simd"],
		["testbed", "Bare metal"],
		["measure", "Throughput"],
	] as const) {
		await page
			.getByRole("button", { name: `Filter by ${filter}, any` })
			.click();
		await page.getByRole("menuitemradio", { name }).click();
		await expect
			.element(
				page.getByRole("button", { name: `Filter by ${filter}, ${name}` }),
			)
			.toBeInTheDocument();
	}
	await expect
		.poll(() => requests.some((url) => url.searchParams.has("measure")))
		.toBe(true);
	await wait(200);
	expect(requests.filter((url) => named.test(url.pathname))).toEqual([]);
});

// Kills a cold link's chip that shows the UUID until its name answers.
test("a filter in a cold link holds its chip's place until its name answers", async () => {
	let answer: (() => void) | undefined;
	const { api } = fakeApi(async (url) => {
		if (url.pathname.endsWith(`/branches/${MAIN}`)) {
			await new Promise<void>((resolve) => {
				answer = resolve;
			});
			return { data: { uuid: MAIN, name: "main", slug: "main" } };
		}
		return batch(0, 1, 1);
	});
	mount(api, client(), `${PATH}?branch=${MAIN}`);

	await expect
		.element(page.getByRole("button", { name: "Filter by branch, …" }))
		.toBeVisible();
	expect(page.getByText(MAIN).query()).toBeNull();
	answer?.();
	await expect
		.element(page.getByRole("button", { name: "Filter by branch, main" }))
		.toBeVisible();
});

// Kills a row that does not say its testbed was archived, wide or on a phone,
// where the testbed's own cell folds away and a line's end is cut off.
test("a row says its testbed was archived, on a phone too", async () => {
	const { api } = fakeApi(() => ({
		data: thresholdsFixture([{}], {
			testbeds: [
				{
					uuid: "testbed-uuid",
					name: "ubuntu-old",
					slug: "ubuntu-old",
					archived: Date.parse("2026-09-08T12:00:00Z"),
				},
			],
		}),
	}));
	mount(api, client());
	await expect
		.element(table().getByRole("row").nth(1).getByRole("cell").nth(1))
		.toHaveTextContent(/^ubuntu-old · archived Sep 8(, 2026)?$/);
	dispose?.();
	document.body.replaceChildren();

	await page.viewport(390, 844);
	mount(api, client());
	await expect.element(table()).toBeVisible();
	const line = document.querySelector<HTMLElement>(
		'tbody tr.th-row > td[data-fold="l2"]',
	);
	const note = [
		...(line?.querySelectorAll<HTMLElement>(".th-from") ?? []),
	].find((span) =>
		/^testbed archived Sep 8(, 2026)? · $/.test(span.textContent ?? ""),
	);
	expect(note?.getBoundingClientRect().right).toBeLessThanOrEqual(
		line?.getBoundingClientRect().right ?? 0,
	);
});

// Kills a Status control that keeps its own width on a phone.
test("on a phone, Status spans the page", async () => {
	await page.viewport(390, 844);
	const { api } = fakeApi(() => batch(0, 1, 1));
	mount(api, client());
	const status = page.getByRole("radiogroup", { name: "Status" });
	await expect.element(status).toBeVisible();
	expect(status.element().getBoundingClientRect().width).toBeGreaterThan(300);
});

// Kills an Archived view whose filter menus leave out the archived branch
// that archived its thresholds, archived options out of name order, and an
// Active view that asks for or lists archived ones.
test("under Archived, a filter menu lists archived branches too, in name order", async () => {
	const { api, requests } = fakeApi((url) => {
		if (url.pathname.endsWith("/branches")) {
			return {
				data:
					url.searchParams.get("archived") === "true"
						? [{ uuid: LEGACY, name: "legacy", slug: "legacy" }]
						: [{ uuid: MAIN, name: "main", slug: "main" }],
			};
		}
		return batch(0, 1, 1);
	});
	mount(api, client());
	await expect.element(table()).toBeVisible();
	const options = () =>
		page
			.getByRole("menuitemradio")
			.all()
			.map((item) => item.element().textContent);
	const archivedAsked = () =>
		requests
			.filter((url) => url.pathname.endsWith("/branches"))
			.map((url) => url.searchParams.get("archived"));

	await page.getByRole("button", { name: "Filter by branch, any" }).click();
	await expect
		.element(page.getByRole("menuitemradio", { name: "main" }))
		.toBeVisible();
	expect(options()).toEqual(["Any branch", "main"]);
	expect(archivedAsked()).toEqual([null]);
	await userEvent.keyboard("{Escape}");

	await page.getByRole("radio", { name: "Archived" }).click();
	await page.getByRole("button", { name: "Filter by branch, any" }).click();
	await expect
		.element(page.getByRole("menuitemradio", { name: "legacy" }))
		.toBeVisible();
	expect(options()).toEqual(["Any branch", "legacy", "main"]);
	expect(archivedAsked()).toEqual([null, "true"]);
});
