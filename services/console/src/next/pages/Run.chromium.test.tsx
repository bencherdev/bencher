import "@bencherdev/ui/styles.css";
import "../styles/console.css";
import "../reports/reports.css";
import "../report/report.css";
import "../dimensions/dimensions.css";
import { MemoryRouter, Route, createMemoryHistory } from "@solidjs/router";
import { type QueryClient, QueryClientProvider } from "@tanstack/solid-query";
import { Suspense } from "solid-js";
import { render } from "solid-js/web";
import { afterEach, beforeEach, expect, test } from "vitest";
import { page } from "vitest/browser";
import { type Api, ApiError } from "../api";
import { createQueryClient } from "../cache";
import { ProjectContext } from "../project";
import { fakeApi } from "../reports/testing";
import { screenBatch } from "../reports/virtual";
import { runPageFixture, runReportFixture } from "../runs/testing";
import { expectRowOpens } from "../ui/testing";
import Run from "./Run";

const RUN = "00000000-0000-4000-8000-000000000000";
const PATH = `/hashbrown/runs/${RUN}`;

let dispose: (() => void) | undefined;

const mount = (api: Api, path = PATH) => {
	const history = createMemoryHistory();
	history.set({ value: path });
	const root = document.createElement("div");
	root.className = "console";
	document.body.append(root);
	dispose = render(
		() => (
			<QueryClientProvider client={client()}>
				<MemoryRouter
					history={history}
					root={(props) => (
						<ProjectContext.Provider value={{ api, slug: () => "hashbrown" }}>
							<Suspense fallback={<p>Suspended</p>}>{props.children}</Suspense>
						</ProjectContext.Provider>
					)}
				>
					<Route path="/:project/runs/:run" component={Run} />
				</MemoryRouter>
			</QueryClientProvider>
		),
		root,
	);
	return history;
};

/** The production cache, but a refusal fails at once. */
const client = (): QueryClient => {
	const cache = createQueryClient(() => {});
	cache.setDefaultOptions({
		queries: { ...cache.getDefaultOptions().queries, retry: false },
	});
	return cache;
};

let perPage = 0;
const reports = (from: number, count: number) =>
	Array.from({ length: count }, (_, index) => runReportFixture(from + index));
const table = () =>
	page.getByRole("table", { name: "Reports in this run, by start" });
const drawn = () =>
	[...document.querySelectorAll<HTMLElement>("tbody tr[aria-rowindex]")].map(
		(row) => Number(row.getAttribute("aria-rowindex")),
	);
const cells = (row: number) =>
	[...document.querySelectorAll(`tr[aria-rowindex="${row}"] td`)].map((cell) =>
		cell.textContent?.replace(/\s+/g, " ").trim(),
	);
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

// Kills a heading that names the run by anything but its branch and head, an
// alerts count of the active alerts rather than every one, a sub line whose
// counts or time are read from the page rather than the run, cells that read
// the wrong field, a number taken from the row's place rather than the report's
// order in the run, and a row that opens anything but its report.
test("the heading names the run and the table reads its reports in start order", async () => {
	const { api, requests } = fakeApi(() => ({
		data: runPageFixture(
			[
				runReportFixture(0, { order: 4 }),
				runReportFixture(1, {
					order: 5,
					testbed: {
						uuid: "m",
						name: "macos-latest",
						slug: "macos-latest",
					} as never,
					adapter: "magic" as never,
					lines: 6,
					alerts: { total: 2, active: 1 },
					end_time: Date.parse("2026-09-13T21:21:13Z"),
					author: { kind: "key", name: "ci-matrix" } as never,
				}),
			],
			{
				total: 8,
				testbeds: 2,
				lines: 96,
				alerts: { total: 3, active: 1 },
			},
		),
	}));
	mount(api);

	const heading = page.getByRole("heading", { level: 1 });
	await expect.element(heading).toHaveTextContent("main · 9c1f2e4");
	await expect.element(page.getByText("3 alerts")).toBeVisible();
	expect(document.querySelector(".rp-sub")?.textContent).toMatch(
		/^Sep 1[34], 2026, \d\d:\d6 · version 248 · 8 reports, 2 testbeds · 96 lines · took 14m 08s$/,
	);
	expect(document.querySelector(".count")?.textContent).toBe("8");
	expect(cells(2)).toEqual([
		"4",
		"ubuntu-latest",
		"json",
		expect.stringMatching(/^\d\d:\d\d:00$/),
		"36 lines",
		"0",
		"2m 00s",
		"Muriel Bagge",
	]);
	expect(cells(3)).toEqual([
		"5",
		"macos-latest",
		"magic",
		expect.stringMatching(/^\d\d:\d\d:01$/),
		"6 lines",
		"1 active 2 total",
		"5m 12s",
		"ci-matrix",
	]);
	const link = table().getByRole("row").nth(2).getByRole("link");
	await expect
		.element(link)
		.toHaveAttribute(
			"href",
			"/next/console/projects/hashbrown/reports/00000000-0000-4000-8000-000000001001",
		);
	expect(link.element().getAttribute("aria-label")).toMatch(
		/^macos-latest, report 5, started \d\d:\d\d:01$/,
	);
	expect(requests[0]?.pathname).toBe(
		`/v0/console/projects/hashbrown/runs/${RUN}`,
	);
	for (const row of document.querySelectorAll("tbody tr[aria-rowindex]")) {
		expectRowOpens(
			row,
			row.querySelector("a")?.getAttribute("href") ?? "no link",
		);
	}
});

// Kills a run without a hash named blank.
test("a run without a hash is named by its version", async () => {
	const { api } = fakeApi(() => ({
		data: runPageFixture([runReportFixture(0)], {
			hash: undefined,
			number: 252,
		}),
	}));
	mount(api);
	await expect
		.element(page.getByRole("heading", { level: 1 }))
		.toHaveTextContent("main · v252");
});

// Kills a long run drawn whole or never paged, a next page asked for at once,
// and a second page drawn anywhere but after the first.
test("a long run loads its next page of reports as the reader scrolls", async () => {
	const total = 2 * perPage;
	const { api, requests } = fakeApi((url) => {
		const batch = Number(url.searchParams.get("page"));
		return {
			data: runPageFixture(reports((batch - 1) * perPage, perPage), {
				total,
			}),
		};
	});
	mount(api);
	await expect.element(table()).toBeVisible();
	await expect
		.element(table())
		.toHaveAttribute("aria-rowcount", String(total + 1));
	await wait(200);
	expect(requests.map((url) => url.searchParams.get("page"))).toEqual(["1"]);
	expect(requests[0]?.searchParams.get("per_page")).toBe(String(perPage));
	expect(drawn().length).toBeLessThan(perPage);

	await expect
		.poll(() => {
			toBottom();
			return drawn().at(-1);
		})
		.toBe(2 * perPage + 1);
	expect(requests.map((url) => url.searchParams.get("page"))).toEqual([
		"1",
		"2",
	]);
	expect(cells(2 * perPage + 1)[0]).toBe(String(2 * perPage));
});

// Kills another run that starts with as many pages as the last one had loaded,
// which asks the API for pages the reader never scrolled to.
test("another run starts again from its first page", async () => {
	const other = "00000000-0000-4000-8000-0000000000ff";
	const { api, requests } = fakeApi((url) => {
		const batch = Number(url.searchParams.get("page"));
		return {
			data: runPageFixture(reports((batch - 1) * perPage, perPage), {
				total: 2 * perPage,
			}),
		};
	});
	const history = mount(api);
	await expect.element(table()).toBeVisible();
	await expect
		.poll(() => {
			toBottom();
			return requests.length;
		})
		.toBe(2);
	window.scrollTo({ top: 0, behavior: "instant" });
	await wait(100);

	history.set({ value: `/hashbrown/runs/${other}` });
	await expect
		.poll(() => requests.some((url) => url.pathname.endsWith(other)))
		.toBe(true);
	await wait(300);
	expect(
		requests
			.filter((url) => url.pathname.endsWith(other))
			.map((url) => url.searchParams.get("page")),
	).toEqual(["1"]);
});

// Kills a missing run drawn as a page that never loads, and an address that
// cannot be a run asked for at all.
test("a run the API cannot find, or that cannot be one, is not found", async () => {
	const { api, requests } = fakeApi(() =>
		Promise.reject(new ApiError(404, "not_found", "Version not found")),
	);
	mount(api);
	await expect
		.element(page.getByRole("heading", { level: 1, name: "Run not found" }))
		.toBeVisible();
	await expect
		.element(page.getByRole("link", { name: "Runs" }))
		.toHaveAttribute("href", "/next/console/projects/hashbrown/runs");
	dispose?.();
	document.body.replaceChildren();
	requests.length = 0;

	mount(api, "/hashbrown/runs/nope");
	await expect
		.element(page.getByRole("heading", { level: 1, name: "Run not found" }))
		.toBeVisible();
	expect(requests).toEqual([]);
});

// Kills a run that failed to load shown as missing or as nothing, and a Retry
// that does not ask again.
test("a run the API did not answer shows a banner whose Retry loads it", async () => {
	let fail = true;
	const { api } = fakeApi(() =>
		fail
			? Promise.reject(new ApiError(503, "server", "down"))
			: { data: runPageFixture([runReportFixture(0)]) },
	);
	mount(api);
	const banner = page.getByRole("alert");
	await expect.element(banner).toHaveTextContent("This run did not load");
	fail = false;
	await banner.getByRole("button", { name: "Retry" }).click();
	await expect.element(table()).toBeVisible();
});

// Kills a phone row that keeps every column, is shorter than a touch target,
// loses its number, or that only its text opens.
test("phone rows fold into two lines beside their number and open the report anywhere", async () => {
	await page.viewport(390, 844);
	const { api } = fakeApi(() => ({
		data: runPageFixture([runReportFixture(0)]),
	}));
	mount(api);
	await expect.element(table()).toBeVisible();
	const row = document.querySelector("tr[aria-rowindex='2']") as HTMLElement;
	await expect.poll(() => row.getBoundingClientRect().height).toBe(60);
	expect(document.querySelector("thead")?.checkVisibility()).toBe(false);
	const order = row.querySelector(".rn-ord") as HTMLElement;
	const link = row.querySelector("a") as HTMLElement;
	expect(order.checkVisibility()).toBe(true);
	expect(order.getBoundingClientRect().right).toBeLessThanOrEqual(
		link.getBoundingClientRect().left,
	);
	const box = row.getBoundingClientRect();
	const hit = document.elementFromPoint(box.right - 4, box.bottom - 4);
	expect(hit?.closest("a")?.getAttribute("href")).toBe(
		"/next/console/projects/hashbrown/reports/00000000-0000-4000-8000-000000001000",
	);
	expect(document.querySelector(".rp-sub")?.textContent).toMatch(
		/ · v248 · 1 report · 36 lines · took /,
	);
});
