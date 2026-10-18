import "@bencherdev/ui/styles.css";
import "../styles/console.css";
import "../reports/reports.css";
import "../report/report.css";
import "../dimensions/dimensions.css";
import "../plot/plot.css";
import "../thresholds/thresholds.css";
import { MemoryRouter, Route, createMemoryHistory } from "@solidjs/router";
import { type QueryClient, QueryClientProvider } from "@tanstack/solid-query";
import { Suspense } from "solid-js";
import { render } from "solid-js/web";
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { page, userEvent } from "vitest/browser";
import type {
	JsonConsoleJob,
	JsonConsoleRunJobs,
	JsonConsoleRunReport,
} from "../../types/bencher";
import { alertId, alertsFixture, reportId } from "../alerts/testing";
import { type Api, ApiError } from "../api";
import { createQueryClient } from "../cache";
import { NEXT_PROJECTS } from "../paths";
import { ProjectContext } from "../project";
import { fakeApi } from "../reports/testing";
import { screenBatch as reportScreenBatch } from "../report/query";
import { screenBatch } from "../reports/virtual";
import {
	cutCells,
	cutHeaders,
	likelyReport,
	overrun,
	runPageFixture,
	runReportFixture,
} from "../runs/testing";
import { bootstrapOf } from "../settings/testing";
import {
	columnEdges,
	expectNameStartsWithText,
	expectRowOpens,
} from "../ui/testing";
import RunReports from "../runs/RunReports";
import Run from "./Run";

const RUN = "00000000-0000-4000-8000-000000000000";
const PATH = `/hashbrown/runs/${RUN}`;

let dispose: (() => void) | undefined;
let queryClient: QueryClient;

const mount = (api: Api, path = PATH, base?: string) => {
	const history = createMemoryHistory();
	history.set({ value: path });
	const root = document.createElement("div");
	root.className = "console";
	document.body.append(root);
	// One client, since a prop read again would make another.
	queryClient = client();
	dispose = render(
		() => (
			<QueryClientProvider client={queryClient}>
				<MemoryRouter
					history={history}
					{...(base === undefined ? {} : { base })}
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
	vi.useRealTimers();
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

const NEIGHBOR = "00000000-0000-4000-8000-0000000000aa";
/** The run before or after this one, as its heading links it. */
const neighbor = () => ({
	uuid: NEIGHBOR,
	number: 247,
	hash: `4e02c1d${"0".repeat(33)}`,
	start_time: Date.parse("2026-09-11T20:58:00Z"),
});

// Kills a Previous that leads to another run or names it by anything but its
// first start, branch, and head, a Next drawn as a link while there is none,
// and a name that does not start with what the control shows.
test("Previous and Next step to the runs on either side on its branch", async () => {
	const { api } = fakeApi(() => ({
		data: runPageFixture([runReportFixture(0)], { previous: neighbor() }),
	}));
	mount(api);
	const previous = page.getByRole("link", { name: /^Previous / });
	await expect
		.element(previous)
		.toHaveAttribute(
			"href",
			`/next/console/projects/hashbrown/runs/${NEIGHBOR}`,
		);
	expect(previous.element().getAttribute("aria-label")).toMatch(
		/^Previous Sep 1[01], \d\d:\d8: the run on main, 4e02c1d$/,
	);
	expectNameStartsWithText(previous.element());
	await expect
		.element(page.getByRole("button", { name: "Next" }))
		.toBeDisabled();
});

// Kills a step to another run that is not asked for before the click, asks
// for another batch or size than that run's page will, or asks again on the click.
test("hovering Next starts that run's first batch, so the move asks nothing more", async () => {
	const { api, requests } = fakeApi((url) =>
		url.pathname.endsWith(NEIGHBOR)
			? {
					data: runPageFixture([runReportFixture(0)], {
						uuid: NEIGHBOR,
						hash: `4e02c1d${"0".repeat(33)}`,
					}),
				}
			: {
					data: runPageFixture([runReportFixture(0)], { next: neighbor() }),
				},
	);
	mount(api, `${NEXT_PROJECTS}${PATH}`, NEXT_PROJECTS);
	const next = page.getByRole("link", { name: /^Next / });
	await next.hover();
	const asked = () => requests.filter((url) => url.pathname.endsWith(NEIGHBOR));
	await expect.poll(() => asked().length).toBe(1);
	expect(asked()[0]?.searchParams.get("page")).toBe("1");
	expect(asked()[0]?.searchParams.get("per_page")).toBe(String(perPage));
	await next.click();
	await expect
		.element(page.getByRole("heading", { level: 1 }))
		.toHaveTextContent("main · 4e02c1d");
	expect(asked()).toHaveLength(1);
});

// Kills a report row whose page is not asked for before the click, or is
// asked for in another view or batch size than the report's page asks.
test("hovering a report starts its page's first batch in the default view", async () => {
	const { api, requests } = fakeApi((url) => ({
		data: url.pathname.includes("/reports/")
			? {}
			: runPageFixture([runReportFixture(0)]),
	}));
	mount(api);
	await table().getByRole("link").first().hover();
	const asked = () =>
		requests.filter((url) => url.pathname.includes("/reports/"));
	await expect.poll(() => asked().length).toBe(1);
	const [report] = asked();
	expect(report?.pathname).toBe(
		`/v0/console/projects/hashbrown/reports/${runReportFixture(0).uuid}`,
	);
	expect(report?.searchParams.toString()).toBe(
		`window=28&page=1&per_page=${reportScreenBatch()}`,
	);
});

const alerts = () =>
	page.getByRole("table", { name: "Alerts in run 9c1f2e4, by report" });

/** A run whose second report raised an active alert and whose fifth raised one since dismissed. */
const alerting = () =>
	runPageFixture([runReportFixture(0)], {
		total: 8,
		alerts: { total: 2, active: 1 },
		alerting: alertsFixture([
			{ report: 2, order: 2, testbed: 1, alerts: [{ alert: 1 }] },
			{
				report: 5,
				order: 5,
				alerts: [{ alert: 2, benchmark: 1, status: "dismissed" }],
			},
		]),
	});

// Kills an alert row that names another report or its place in the run, a
// report or alert link that leads elsewhere, a dismissed alert drawn as
// alerting, a count that is not the run's, and a Report column that takes the
// measure's place.
test("the run's alerts draw each line with the report it came from", async () => {
	const { api } = fakeApi(() => ({ data: alerting() }));
	mount(api);
	const section = page.getByRole("region", { name: "Alerts in this run" });
	await expect.element(section).toBeVisible();
	expect(section.element().querySelector(".dm-section-head")?.textContent).toBe(
		"Alerts in this run2",
	);
	await expect.element(alerts()).toBeVisible();
	const headers = [...alerts().element().querySelectorAll("th")].map(
		(cell) => cell.textContent,
	);
	expect(headers.slice(2, 5)).toEqual(["Line", "Measure", "Report"]);
	const report = alerts().getByRole("link", { name: /^#2 · macos-latest/ });
	await expect
		.element(report)
		.toHaveAttribute(
			"href",
			`/next/console/projects/hashbrown/reports/${reportId(2)}`,
		);
	expectNameStartsWithText(report.element());
	await expect
		.element(alerts().getByRole("link", { name: /^#5 · ubuntu-latest/ }))
		.toHaveAttribute(
			"href",
			`/next/console/projects/hashbrown/reports/${reportId(5)}`,
		);
	await expect
		.element(
			alerts()
				.getByRole("link", { name: /open the alert$/ })
				.first(),
		)
		.toHaveAttribute(
			"href",
			`/next/console/projects/hashbrown/alerts/${alertId(1)}`,
		);
	const rows = [...alerts().element().querySelectorAll("tbody tr.lr")];
	expect(rows.map((row) => row.className)).toEqual([
		"lr lr-alerting",
		"lr lr-dim",
	]);
});

// Kills a row that opens its plot anywhere but in place, and an Open in
// Explore that leaves out the report the line came from.
test("an alert row opens its plot in place and its line in Explore from its report", async () => {
	const { api } = fakeApi(() => ({ data: alerting() }));
	mount(api);
	await alerts()
		.getByRole("button", { name: /^Expand blake3 / })
		.click();
	await expect
		.element(page.getByRole("region", { name: /^blake3 .*, full plot$/ }))
		.toBeVisible();
	const explore = page.getByRole("link", { name: /^Open in Explore: blake3 / });
	await expect.element(explore).toBeVisible();
	const href = new URL(
		explore.element().getAttribute("href") ?? "",
		"http://console.test",
	);
	expect(href.pathname).toBe("/next/console/projects/hashbrown/explore");
	expect(href.search).toContain(reportId(2));
});

// Kills an alerts section drawn for a run that raised none.
test("a run that raised no alerts draws no alerts section", async () => {
	const { api } = fakeApi(() => ({
		data: runPageFixture([runReportFixture(0)], {
			alerting: alertsFixture([]),
		}),
	}));
	mount(api);
	await expect.element(table()).toBeVisible();
	expect(
		document.querySelector("[aria-label='Alerts in this run']"),
	).toBeNull();
});

// Kills a count that claims every alert is drawn when the section holds a page of them.
test("a run with more alerts than the section draws counts how many it draws", async () => {
	const { api } = fakeApi(() => ({
		data: runPageFixture([runReportFixture(0)], {
			alerting: alertsFixture(
				[{ report: 2, order: 2, alerts: [{ alert: 1 }] }],
				{
					total: 70,
				},
			),
		}),
	}));
	mount(api);
	const section = page.getByRole("region", { name: "Alerts in this run" });
	await expect.element(section).toBeVisible();
	expect(section.element().querySelector(".count")?.textContent).toBe(
		"1 of 70",
	);
});

// Kills a phone alert row that drops its report or its measure, one whose
// report's testbed pushes the measure out of sight, one that grows past the
// height the list lays its rows out by, and a report there drawn as a link too
// small to touch.
test("a phone alert row names its report by place, as text, beside its measure", async () => {
	await page.viewport(390, 844);
	const { api } = fakeApi(() => ({ data: alerting() }));
	mount(api);
	await expect.element(alerts()).toBeVisible();
	const row = alerts().element().querySelector<HTMLElement>("tr.lrn");
	const line = row?.querySelector<HTMLElement>(".lrn-l2");
	expect(line?.textContent?.replace(/\s+/g, " ")).toMatch(
		/^#2 · macos-latest·Latency/,
	);
	// The measure in sight, and its history starting beside it.
	const end = line?.getBoundingClientRect().right ?? 0;
	const measure = [...(line?.children ?? [])].find(
		(child) => child.textContent === "Latency",
	);
	expect(measure?.getBoundingClientRect().right).toBeLessThanOrEqual(end);
	expect(
		measure?.nextElementSibling?.getBoundingClientRect().left,
	).toBeLessThan(end);
	expect(row?.getBoundingClientRect().height).toBe(64);
	expect(
		alerts()
			.getByRole("link", { name: /^#2 · macos-latest/ })
			.elements(),
	).toHaveLength(0);
});

// Kills a phone heading whose Previous and Next fall below a touch target or
// run off the screen.
test("on a phone, Previous and Next are touch targets inside the screen", async () => {
	await page.viewport(390, 844);
	const { api } = fakeApi(() => ({
		data: runPageFixture([runReportFixture(0)], { previous: neighbor() }),
	}));
	mount(api);
	const previous = page.getByRole("link", { name: /^Previous / });
	await expect.element(previous).toBeVisible();
	for (const control of [
		previous.element(),
		page.getByRole("button", { name: "Next" }).element(),
	]) {
		const box = control.getBoundingClientRect();
		expect(box.height).toBeGreaterThanOrEqual(44);
		expect(box.right).toBeLessThanOrEqual(390);
	}
});

// Kills an alert's Report link that waits for the click to ask for its report's first batch.
test("moving onto an alert's report starts its page's first batch", async () => {
	const { api, requests } = fakeApi((url) => ({
		data: url.pathname.includes("/reports/") ? {} : alerting(),
	}));
	mount(api);
	await alerts()
		.getByRole("link", { name: /^#2 · macos-latest/ })
		.hover();
	await expect
		.poll(() =>
			requests.some((url) => url.pathname.endsWith(`/reports/${reportId(2)}`)),
		)
		.toBe(true);
});

// Kills a count that adds up the reports that alerted rather than the alerts drawn.
test("the count counts the alerts drawn, several to a report", async () => {
	const { api } = fakeApi(() => ({
		data: runPageFixture([runReportFixture(0)], {
			alerting: alertsFixture(
				[
					{
						report: 2,
						order: 2,
						alerts: [{ alert: 1 }, { alert: 2, benchmark: 1 }],
					},
				],
				{ total: 70 },
			),
		}),
	}));
	mount(api);
	const section = page.getByRole("region", { name: "Alerts in this run" });
	await expect.element(section).toBeVisible();
	expect(section.element().querySelector(".count")?.textContent).toBe(
		"2 of 70",
	);
});

/** The run's alerts, read by a reader who may or may not change them. */
const mountAlerting = (edit: boolean) => {
	const { api } = fakeApi((url) => ({
		data:
			url.pathname === "/v0/console/projects/hashbrown"
				? bootstrapOf({ edit })
				: alerting(),
	}));
	mount(api);
};

/** Opens the first alert row's plot and pins its readout on the alert, the last point. */
const pinAlert = async () => {
	await alerts()
		.getByRole("button", { name: /^Expand blake3 / })
		.click();
	const region = page.getByRole("region", { name: /^blake3 .*, full plot$/ });
	const plot = region.getByRole("group", { name: /^1 line/ });
	await expect.element(plot).toBeVisible();
	(plot.element() as HTMLElement).focus();
	await userEvent.keyboard("{End}");
	const readout = region.getByRole("status");
	await expect
		.element(readout.getByRole("link", { name: /^Alert: blake3 / }))
		.toBeVisible();
	return readout;
};

// Kills an alert row's readout that offers a change to a reader who may not
// make it, or none to one who may.
test("an alert row's readout offers a change only to a reader who may make it", async () => {
	mountAlerting(true);
	await expect
		.element(
			(await pinAlert()).getByRole("button", {
				name: /^Dismiss the alert on /,
			}),
		)
		.toBeVisible();
	dispose?.();
	document.body.replaceChildren();

	mountAlerting(false);
	const readout = await pinAlert();
	expect(
		readout.getByRole("button", { name: /the alert on/ }).elements(),
	).toHaveLength(0);
});

// Kills a row whose cells sit under another column's header, and an opened
// plot that spans fewer columns than the row.
test("each alert cell sits under its header, and an opened plot spans the row", async () => {
	const { api } = fakeApi(() => ({ data: alerting() }));
	mount(api);
	await expect.element(alerts()).toBeVisible();
	const headers = [...alerts().element().querySelectorAll("thead th")];
	const cells = [
		...(alerts().element().querySelector("tbody tr.lr")?.children ?? []),
	];
	expect(cells).toHaveLength(headers.length);
	headers.forEach((header, index) => {
		expect(
			Math.round(cells[index]?.getBoundingClientRect().left ?? -1),
			header.textContent ?? "",
		).toBe(Math.round(header.getBoundingClientRect().left));
	});
	const under = (label: string) =>
		cells[headers.findIndex((header) => header.textContent === label)]
			?.textContent;
	expect(under("Measure")).toBe("Latency");
	expect(under("Report")).toMatch(/^#\d · /);
	await alerts()
		.getByRole("button", { name: /^Expand blake3 / })
		.click();
	await expect
		.poll(
			() =>
				alerts().element().querySelector<HTMLTableCellElement>("tr.lr-exrow td")
					?.colSpan,
		)
		.toBe(headers.length);
});

// Kills a Line column that the Report column squeezes on a small laptop.
test("at 1024 px the Report column leaves the Line column its room", async () => {
	await page.viewport(1024, 768);
	const { api } = fakeApi(() => ({ data: alerting() }));
	mount(api);
	await expect.element(alerts()).toBeVisible();
	const width = (column: number) =>
		alerts()
			.element()
			.querySelector(`thead th:nth-child(${column})`)
			?.getBoundingClientRect().width ?? 0;
	expect(width(3)).toBeGreaterThan(width(5));
});

// Kills a step that prefetches when the page changes under a still pointer,
// which is not the reader moving toward it.
test("a pointer that has not moved starts no prefetch", async () => {
	const { api, requests } = fakeApi(() => ({
		data: runPageFixture([runReportFixture(0)], { next: neighbor() }),
	}));
	mount(api);
	const next = page.getByRole("link", { name: /^Next / });
	await expect.element(next).toBeVisible();
	const asked = () => requests.filter((url) => url.pathname.endsWith(NEIGHBOR));
	const move = (movementX: number) =>
		next
			.element()
			.dispatchEvent(
				new PointerEvent("pointermove", { bubbles: true, movementX }),
			);
	move(0);
	await wait(100);
	expect(asked()).toHaveLength(0);
	move(4);
	await expect.poll(() => asked().length).toBe(1);
});

/** A run's reports whose later ones run on a testbed of a longer name and alerted, which an auto layout would widen its columns for. */
const variedReports = (from: number, count: number, longFrom = 20) =>
	Array.from({ length: count }, (_, index) =>
		runReportFixture(
			from + index,
			from + index < longFrom
				? {}
				: {
						testbed: {
							uuid: "t",
							name: "ubuntu-22.04-arm64-large-runner",
							slug: "t",
						} as never,
						adapter: "rust_criterion" as never,
						lines: 123_456,
						alerts: { total: 120, active: 64 },
						author: {
							kind: "key",
							name: "a project key with a long name",
						} as never,
					},
		),
	);

// Kills a run's reports table laid out by the rows drawn (an auto layout),
// whose columns shift as rows of other widths scroll into view, at desktop and
// at 768 px.
test("the reports table's columns hold still as rows of other widths scroll into view", async () => {
	for (const width of [1280, 768]) {
		await page.viewport(width, 720);
		const batch = screenBatch();
		const { api } = fakeApi((url) => {
			const at = Number(url.searchParams.get("page"));
			return {
				// The first batch alone is drawn first, so the longer rows arrive with the scroll.
				data: runPageFixture(variedReports((at - 1) * batch, batch, batch), {
					total: 3 * batch,
				}),
			};
		});
		mount(api);
		await expect.element(table()).toBeVisible();
		const before = columnEdges(table().element());
		expect(before.cells).toEqual(before.headers);
		await expect
			.poll(() => {
				toBottom();
				return drawn().at(-1);
			})
			.toBe(3 * batch + 1);
		const after = columnEdges(table().element());
		expect(after.headers).toEqual(before.headers);
		expect(after.cells).toEqual(before.headers);
		window.scrollTo({ top: 0, behavior: "instant" });
		dispose?.();
		document.body.replaceChildren();
	}
});

// Kills a run's reports table that does not fold on a phone (an auto layout
// there), whose rows' cells follow the widest rows drawn beside them.
test("on a phone, each report row folds on its own, whatever rows are drawn beside it", async () => {
	await page.viewport(390, 720);
	const firstRow = async (reports: JsonConsoleRunReport[]) => {
		const { api } = fakeApi(() => ({
			data: runPageFixture(reports, { total: reports.length }),
		}));
		mount(api);
		await expect.element(table()).toBeVisible();
		const { cells } = columnEdges(table().element());
		dispose?.();
		document.body.replaceChildren();
		return cells;
	};
	expect(
		await firstRow([...variedReports(0, 1), ...variedReports(20, 19)]),
	).toEqual(await firstRow(variedReports(0, 20)));
});

// Kills a run's reports table that needs more room than its columns do (a
// minimum width past a likely report's), which scrolls sideways below a
// laptop where every column fits, and one that cuts off a header at a
// tablet's 768 px, where it scrolls rather than cut text (ruling 407).
test("below a laptop the reports table scrolls only where its columns need the room", async () => {
	for (const width of [768, 960]) {
		await page.viewport(width, 1024);
		const { api } = fakeApi(() => ({
			data: runPageFixture(variedReports(0, 8, 4), { total: 8 }),
		}));
		mount(api);
		await expect.element(table()).toBeVisible();
		await document.fonts.ready;
		expect(cutHeaders(table().element()), `at ${width} px`).toEqual([]);
		const scroll = table().element().closest(".ui-table-scroll") as HTMLElement;
		if (width === 960) {
			expect(scroll.scrollWidth).toBe(scroll.clientWidth);
		}
		dispose?.();
		document.body.replaceChildren();
	}
});

const START = runReportFixture(0).start_time;
const job = (fields: Partial<JsonConsoleJob>) =>
	({ status: "queued", created: START, ...fields }) as JsonConsoleJob;

// Kills a report's status read from anything but its job, a count drawn where
// nothing landed, a start shown for a job no runner took, a link that leaves
// its job's status out of its name, and a heading without the run's status or
// with a time its jobs have not reached.
test("a run's reports read their jobs' statuses where their counts will land", async () => {
	vi.useFakeTimers({ toFake: ["Date"] });
	vi.setSystemTime(START + 60_000);
	const { api } = fakeApi(() => ({
		data: runPageFixture(
			[
				runReportFixture(0),
				runReportFixture(1, { job: job({}) }),
				runReportFixture(2, {
					job: job({
						status: "running" as JsonConsoleJob["status"],
						started: START + 12_000,
					}),
				}),
				runReportFixture(3, {
					job: job({
						status: "failed" as JsonConsoleJob["status"],
						started: START + 3_000,
						completed: START + 3_000 + 42_000,
					}),
				}),
				runReportFixture(4, {
					job: job({
						status: "canceled" as JsonConsoleJob["status"],
						completed: START + 30_000,
					}),
				}),
			],
			{
				jobs: {
					queued: 1,
					running: 1,
					failed: 1,
					canceled: 1,
					since: START + 1_000,
				} as JsonConsoleRunJobs,
			},
		),
	}));
	mount(api);
	await expect.element(table()).toBeVisible();
	expect(cells(2).slice(3, 7)).toEqual(["21:16:00", "36 lines", "0", "2m 00s"]);
	expect(cells(3).slice(3, 7)).toEqual(["", "-", "-", "Queued"]);
	expect(cells(4).slice(3, 7)).toEqual(["21:16:02", "-", "-", "Running 48s"]);
	expect(cells(5).slice(6, 7)).toEqual(["Failed 42s"]);
	expect(cells(6).slice(6, 7)).toEqual(["Canceled"]);
	await expect
		.element(
			table().getByRole("link", { name: "ubuntu-latest, report 2, Queued" }),
		)
		.toBeVisible();
	const title = document.querySelector(".rp-titlerow")?.textContent;
	expect(title).toContain("Running 59s");
	expect(document.querySelector(".rp-sub")?.textContent).not.toContain("took");
});

/** A running run whose first report raised `count` alerts so far. */
const alertingSoFar = (count: number) =>
	runPageFixture([runReportFixture(0)], {
		total: 2,
		alerts: { total: count, active: count },
		jobs: {
			queued: 0,
			running: 1,
			failed: 0,
			canceled: 0,
			since: START,
		} as JsonConsoleRunJobs,
		...(count > 0 && {
			alerting: alertsFixture([
				{
					report: 2,
					order: 1,
					alerts: Array.from({ length: count }, (_, index) => ({
						alert: index + 1,
						benchmark: index % 2,
					})),
				},
			]),
		}),
	});

// Kills lines that land in a table already showing some and push its rows
// about, a Show that leaves them held, and an empty table that holds its first
// lines rather than filling.
test("a run still taking results holds new alerting lines behind Show once its table shows lines", async () => {
	let count = 1;
	const { api } = fakeApi(() => ({ data: alertingSoFar(count) }));
	mount(api);
	await expect.element(alerts()).toBeVisible();
	const rows = () => alerts().element().querySelectorAll("tbody tr.lr").length;
	expect(rows()).toBe(1);

	count = 2;
	await queryClient.refetchQueries();
	const held = page.getByRole("status").filter({ hasText: "new line" });
	await expect.element(held).toHaveTextContent("1 new lineShow");
	expect(rows()).toBe(1);
	await held.getByRole("button", { name: "Show" }).click();
	await expect.element(held).not.toBeInTheDocument();
	expect(rows()).toBe(2);
});

// Kills an empty table that holds the first lines to land behind Show.
test("a run's first alerting lines fill its table directly", async () => {
	let count = 0;
	const { api } = fakeApi(() => ({ data: alertingSoFar(count) }));
	mount(api);
	await expect.element(table()).toBeVisible();
	count = 2;
	await queryClient.refetchQueries();
	await expect.element(alerts()).toBeVisible();
	await expect
		.poll(() => alerts().element().querySelectorAll("tbody tr.lr").length)
		.toBe(2);
	expect(page.getByRole("button", { name: "Show" }).elements()).toHaveLength(0);
});

// Kills a Status column too narrow for its longest pills or given back its end
// padding, any other column's share cut below what a likely report shows, and
// a minimum width below a laptop too small for one, which cuts its text rather
// than scroll, on the real page from 768 px up (ruling 407).
test("a likely report's every cell is whole, with the longest Running and Failed pills, from 768 px up", async () => {
	vi.useFakeTimers({ toFake: ["Date"] });
	vi.setSystemTime(START + 59 * 60_000);
	const failed = (index: number, took: number) =>
		likelyReport(index, {
			job: job({
				status: "failed" as JsonConsoleJob["status"],
				started: START,
				completed: START + took,
			}),
		});
	for (const width of [768, 1024, 1280]) {
		await page.viewport(width, 1024);
		const { api } = fakeApi(() => ({
			data: runPageFixture([
				likelyReport(0),
				likelyReport(1, {
					job: job({
						status: "running" as JsonConsoleJob["status"],
						started: START,
					}),
				}),
				failed(2, 59 * 60_000 + 59_000),
				failed(3, 25 * 3_600_000 + 59 * 60_000),
			]),
		}));
		mount(api);
		await expect.element(table()).toBeVisible();
		await document.fonts.ready;
		expect(
			[...document.querySelectorAll(".rn-row .rn-st")].map(
				(pill) => pill.textContent,
			),
		).toEqual(["Running 59m", "Failed 59m 59s", "Failed 25h 59m"]);
		expect(
			[...document.querySelectorAll(".rn-row")].flatMap((row) => cutCells(row)),
			`at ${width} px`,
		).toEqual([]);
		expect(cutHeaders(table().element()), `at ${width} px`).toEqual([]);
		dispose?.();
		document.body.replaceChildren();
	}
});

// Kills an Alerts column that cuts a four-digit total at 768 px further than
// it did before the Status column held its pills.
test("at 768 px a report's four-digit alerts total is cut no further than before", async () => {
	await page.viewport(768, 1024);
	const root = document.createElement("div");
	root.className = "console";
	document.body.append(root);
	dispose = render(
		() => (
			<RunReports
				slug="hashbrown"
				reports={[likelyReport(0, { alerts: { total: 9999, active: 999 } })]}
				total={1}
				now={START}
			/>
		),
		root,
	);
	await expect.element(table()).toBeVisible();
	await document.fonts.ready;
	const alerts = document
		.querySelector(".rn-row .reports-alerts")
		?.closest("td") as HTMLElement;
	// Before Status held its pills, Alerts cut it by 32 px here.
	expect(overrun(alerts)).toBeLessThanOrEqual(32);
});

/** A running run of two reports whose first raised the alerts numbered `alerts`, with `fields` over it. */
const alertingRun = (
	alerts: number[],
	fields: Parameters<typeof runPageFixture>[1] = {},
) =>
	runPageFixture([runReportFixture(0)], {
		total: 2,
		alerts: { total: alerts.length, active: alerts.length },
		alerting: alertsFixture([
			{
				report: 2,
				order: 1,
				alerts: alerts.map((alert, index) => ({
					alert,
					benchmark: index % 2,
				})),
			},
		]),
		...fields,
	});

// Kills a move to another run that keeps the last run's shown lines, so every
// alerting line of the new run waits behind Show.
test("moving to another run that alerted draws its lines at once", async () => {
	const { api } = fakeApi((url) => ({
		data: url.pathname.endsWith(NEIGHBOR)
			? alertingRun([11, 12], {
					uuid: NEIGHBOR,
					hash: `4e02c1d${"0".repeat(33)}`,
				})
			: alertingRun([1], { next: neighbor() }),
	}));
	mount(api, `${NEXT_PROJECTS}${PATH}`, NEXT_PROJECTS);
	await expect.element(alerts()).toBeVisible();
	await page.getByRole("link", { name: /^Next / }).click();
	const moved = page.getByRole("table", {
		name: "Alerts in run 4e02c1d, by report",
	});
	await expect.element(moved).toBeVisible();
	await expect
		.poll(() => moved.element().querySelectorAll("tbody tr.lr").length)
		.toBe(2);
	expect(
		page.getByRole("status").filter({ hasText: "new line" }).elements(),
	).toHaveLength(0);
});

// Kills a run page whose running times stand still as its answers come in.
test("a running run's times move on with each answer", async () => {
	vi.useFakeTimers({ toFake: ["Date"] });
	vi.setSystemTime(START + 60_000);
	const { api } = fakeApi(() => ({
		data: runPageFixture(
			[
				runReportFixture(0, {
					job: job({
						status: "running" as JsonConsoleJob["status"],
						started: START + 12_000,
					}),
				}),
			],
			{
				jobs: {
					queued: 0,
					running: 1,
					failed: 0,
					canceled: 0,
					since: START + 12_000,
				} as JsonConsoleRunJobs,
			},
		),
	}));
	mount(api);
	await expect.element(table()).toBeVisible();
	const title = () => document.querySelector(".rp-titlerow")?.textContent;
	expect(title()).toContain("Running 48s");
	expect(cells(2)[6]).toBe("Running 48s");
	vi.setSystemTime(START + 3 * 60_000 + 12_000);
	await queryClient.refetchQueries();
	await expect.poll(title).toContain("Running 3m");
	expect(cells(2)[6]).toBe("Running 3m");
});

// Kills a run's sub line that counts its lines before any have landed.
test("a run with nothing landed counts no lines in its sub line", async () => {
	vi.useFakeTimers({ toFake: ["Date"] });
	vi.setSystemTime(START + 60_000);
	const { api } = fakeApi(() => ({
		data: runPageFixture(
			[
				runReportFixture(0, { job: job({}) }),
				runReportFixture(1, {
					job: job({
						status: "running" as JsonConsoleJob["status"],
						started: START,
					}),
				}),
			],
			{
				jobs: {
					queued: 1,
					running: 1,
					failed: 0,
					canceled: 0,
					since: START,
				} as JsonConsoleRunJobs,
			},
		),
	}));
	mount(api);
	await expect.element(table()).toBeVisible();
	const sub = document.querySelector(".rp-sub")?.textContent ?? "";
	expect(sub).toContain("2 reports");
	expect(sub).not.toMatch(/\bline/);
});
