import "@bencherdev/ui/styles.css";
import "../styles/console.css";
import "../plot/plot.css";
import "../report/report.css";
import { MemoryRouter, Route, createMemoryHistory } from "@solidjs/router";
import { type QueryClient, QueryClientProvider } from "@tanstack/solid-query";
import { Suspense } from "solid-js";
import { render } from "solid-js/web";
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { page, userEvent } from "vitest/browser";
import type {
	JsonConsoleJob,
	JsonConsoleReport,
	JsonConsoleReportRun,
	JsonConsoleRunReportLink,
	JsonConsoleSpec,
} from "../../types/bencher";
import { type Api, ApiError } from "../api";
import { createQueryClient } from "../cache";
import { PROJECTS_PATH } from "../paths";
import { ProjectContext } from "../project";
import { linesOf } from "../report/lines";
import {
	REPORT,
	batchOf,
	lineFixture,
	longReport,
	reportFixture,
} from "../report/testing";
import { decodeQuery } from "../query/query";
import { fakeApi, watchFallback } from "../reports/testing";
import { screenBatch as runScreenBatch } from "../reports/virtual";
import { bootstrapOf } from "../settings/testing";
import { STRESS, expectNameStartsWithText } from "../ui/testing";
import Report from "./Report";

const PATH = `${PROJECTS_PATH}/hashbrown/reports/${REPORT}`;

let dispose: (() => void) | undefined;

const mount = (
	api: Api,
	client: QueryClient,
	path = PATH,
	host = "https://api.bencher.dev",
) => {
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
					base={PROJECTS_PATH}
					root={(props) => (
						<ProjectContext.Provider
							value={{ api, slug: () => "hashbrown", host }}
						>
							<Suspense fallback={<p>Suspended</p>}>{props.children}</Suspense>
						</ProjectContext.Provider>
					)}
				>
					<Route path="/:project/reports/:report" component={Report} />
				</MemoryRouter>
			</QueryClientProvider>
		),
		root,
	);
	return history;
};

/** The production cache, but a refused request fails at once. */
const impatient = () => {
	const cache = createQueryClient(() => {});
	cache.setDefaultOptions({
		queries: { ...cache.getDefaultOptions().queries, retry: false },
	});
	return cache;
};

const isReport = (url: URL) =>
	url.pathname.includes("/v0/console/projects/hashbrown/reports/");
const table = () => page.getByRole("table", { name: /^Lines in report / });

beforeEach(async () => {
	await page.viewport(1280, 720);
});

afterEach(() => {
	dispose?.();
	document.body.replaceChildren();
	vi.useRealTimers();
});

// Kills lines that wait on the shell's request, which would take a second round.
test("the lines load beside the shell's request, not after it", async () => {
	const { api, requests } = fakeApi((url) =>
		isReport(url) ? { data: reportFixture() } : new Promise<never>(() => {}),
	);
	mount(api, impatient());
	await expect.element(table()).toBeVisible();
	expect(requests.filter(isReport)).toHaveLength(1);
});

// Kills a failed report that shows nothing, or a Retry that does not ask again.
test("a report the API did not answer offers Retry, which loads it", async () => {
	let fail = true;
	const { api, requests } = fakeApi((url) => {
		if (!isReport(url)) {
			return { data: bootstrapOf() };
		}
		return fail
			? Promise.reject(new ApiError(503, "server", "down"))
			: { data: reportFixture() };
	});
	mount(api, impatient());
	const banner = page.getByRole("alert");
	await expect.element(banner).toHaveTextContent("This report did not load");
	fail = false;
	await banner.getByRole("button", { name: "Retry" }).click();
	await expect.element(table()).toBeVisible();
	expect(requests.filter(isReport)).toHaveLength(2);
});

// Kills an empty report drawn as a table of nothing, and a filter with no way back.
test("a report with no lines, or none matching the filter, says so", async () => {
	const { api } = fakeApi((url) =>
		isReport(url)
			? { data: reportFixture([], { total: 0, groups: [] }) }
			: { data: bootstrapOf() },
	);
	const none = page.getByRole("heading", { name: "No lines", exact: true });
	const clear = page.getByRole("button", { name: "Clear the filter" });
	mount(api, impatient());
	await expect.element(none).toBeVisible();
	expect(clear.elements()).toHaveLength(0);

	dispose?.();
	document.body.replaceChildren();
	mount(api, impatient(), `${PATH}?search=avx512`);
	await expect.element(none).toBeVisible();
	await clear.click();
	await expect.element(clear).not.toBeInTheDocument();
	await expect.element(none).toBeVisible();
});

// Kills rows that fall back to the skeleton, or take the page out, while a new view loads.
test("a new view keeps the rows on screen, marked busy, until it answers", async () => {
	let answer: (() => void) | undefined;
	const { api } = fakeApi((url) => {
		if (!isReport(url)) {
			return { data: bootstrapOf() };
		}
		if (url.searchParams.get("sort") !== "delta") {
			return { data: reportFixture() };
		}
		return new Promise((resolve) => {
			answer = () =>
				resolve({ data: reportFixture([lineFixture({ value: 30 })]) });
		});
	});
	mount(api, impatient());
	await expect.element(table()).toBeVisible();
	const fallback = watchFallback();

	await page.getByRole("radio", { name: "worst delta first" }).click();
	await expect.poll(() => answer).toBeTruthy();
	await expect.element(table()).toHaveAttribute("aria-busy", "true");
	await expect.element(table()).toHaveTextContent("20.60 ns");
	answer?.();
	await expect.element(table()).toHaveTextContent("30.00 ns");
	await expect.element(table()).not.toHaveAttribute("aria-busy");
	expect(fallback()).toBe(false);
});

const RUN = "00000000-0000-4000-8000-0000000000f0";
const PREVIOUS = "00000000-0000-4000-8000-000000000002";
const NEXT = "00000000-0000-4000-8000-000000000003";
/** The report's run: its fifth report of eight, between the fourth and sixth on macOS. */
const run = (
	fields: {
		[K in keyof JsonConsoleReportRun]?: JsonConsoleReportRun[K] | undefined;
	} = {},
) => ({
	uuid: RUN,
	order: 5,
	total: 8,
	previous: step(PREVIOUS, 4, "2026-09-13T21:15:58Z"),
	next: step(NEXT, 6, "2026-09-13T21:16:03Z"),
	...fields,
});
/** A step through the run to a report on macOS. */
const step = (
	uuid: string,
	order: number,
	start: string,
): JsonConsoleRunReportLink => ({
	uuid,
	order,
	testbed: "macos-latest",
	start_time: Date.parse(start),
});

// Kills a header that keeps the first report's identity after moving to
// another, and a selection carried over from it.
test("moving to the previous report redraws its identity and starts with nothing selected", async () => {
	const { api } = fakeApi((url) => {
		if (!isReport(url)) {
			return { data: bootstrapOf() };
		}
		return url.pathname.endsWith(PREVIOUS)
			? {
					data: reportFixture(undefined, {
						uuid: PREVIOUS,
						testbed: {
							uuid: "macos",
							name: "macos-latest",
							slug: "macos-latest",
						},
						run: run({ order: 4 }),
					} as never),
				}
			: { data: reportFixture(undefined, { run: run() } as never) };
	});
	mount(api, impatient());
	const heading = page.getByRole("heading", { level: 1 });
	await expect.element(heading).toHaveTextContent("main · ubuntu-latest");
	await page.getByRole("checkbox", { name: "Select blake3" }).click();
	await expect.element(page.getByText("1 line selected")).toBeVisible();
	await page.getByRole("link", { name: /^Previous #4/ }).click();
	await expect.element(heading).toHaveTextContent("main · macos-latest");
	await expect.element(page.getByText("0 lines selected")).toBeVisible();
});

// What the rows' controls call the three lines: the parameter that varies, and the measure where it tells two apart.
const LATENCY = "blake3 input_bytes=65536 Latency";
const SMALL = "blake3 input_bytes=1024";
const THROUGHPUT = "blake3 input_bytes=65536 Throughput";

/** A report of three lines: two latencies a threshold checks, and a throughput none does. */
const threeLines = (fields: Partial<JsonConsoleReport> = {}) =>
	reportFixture(
		[
			lineFixture(),
			lineFixture({ variant: 1 }),
			lineFixture({
				measure: 1,
				model: undefined,
				baseline: undefined,
				upper_limit: undefined,
			}),
		],
		fields,
	);

/** An API that answers each request for `report` with the batch it names. */
const pagedApi = (report = longReport(100), fail = (_url: URL) => false) =>
	fakeApi((url) => {
		if (!isReport(url)) {
			return { data: bootstrapOf() };
		}
		return fail(url)
			? Promise.reject(new ApiError(503, "server", "down"))
			: { data: batchOf(report, url) };
	});

const bottom = () =>
	window.scrollTo({
		top: document.documentElement.scrollHeight,
		behavior: "instant",
	});
const pageOf = (url: URL) => url.searchParams.get("page");
const frame = () =>
	new Promise<void>((resolve) => requestAnimationFrame(() => resolve()));

// Kills a header redrawn as a new object whenever the URL changes, which moves the first row under it and drops its focus.
test("opening or closing the first line under a header keeps the focus on its control", async () => {
	const { api } = fakeApi((url) =>
		isReport(url) ? { data: threeLines() } : { data: bootstrapOf() },
	);
	mount(api, impatient());
	const expand = page.getByRole("button", { name: `Expand ${LATENCY}` });
	const collapse = page.getByRole("button", { name: `Collapse ${LATENCY}` });
	await expect.element(expand).toBeVisible();

	(expand.element() as HTMLElement).focus();
	await userEvent.keyboard("{Enter}");
	await expect.element(collapse).toBeVisible();
	await frame();
	expect(document.activeElement).toBe(collapse.element());

	await userEvent.keyboard(" ");
	await expect.element(expand).toBeVisible();
	await frame();
	expect(document.activeElement).toBe(expand.element());

	await expand.click();
	await expect.element(collapse).toBeVisible();
	await frame();
	expect(document.activeElement).toBe(collapse.element());
});

// Kills later batches that follow a view change and ask for its pages before
// its first, and a view returned to that asks again for every batch it once had.
test("a new view after three batches asks only for its first batch, and so does the view returned to", async () => {
	const { api, requests } = pagedApi();
	const client = impatient();
	client.setDefaultOptions({
		queries: { ...client.getDefaultOptions().queries, staleTime: 0 },
	});
	mount(api, client);
	await expect.element(table()).toBeVisible();
	await expect
		.poll(
			() => {
				bottom();
				return requests.filter(isReport).map(pageOf);
			},
			{ timeout: 5_000 },
		)
		.toEqual(["1", "2", "3", "4"]);
	window.scrollTo({ top: 0, behavior: "instant" });
	const before = requests.length;

	await page.getByRole("radio", { name: "worst delta first" }).click();
	await expect.element(table()).not.toHaveAttribute("aria-busy");
	await new Promise((resolve) => setTimeout(resolve, 300));
	const asked = () =>
		requests
			.slice(before)
			.filter(isReport)
			.map((url) => [pageOf(url), url.searchParams.get("sort")]);
	expect(asked()).toEqual([["1", "delta"]]);

	await page.getByRole("radio", { name: "alerting first" }).click();
	await expect.element(table()).not.toHaveAttribute("aria-busy");
	await new Promise((resolve) => setTimeout(resolve, 300));
	expect(asked()).toEqual([
		["1", "delta"],
		["1", null],
	]);
});

// Kills a later batch whose failure offers a Retry that asks for nothing.
test("a later batch that did not load offers Retry, which loads it", async () => {
	let down = true;
	const { api, requests } = pagedApi(
		longReport(60),
		(url) => down && pageOf(url) === "2",
	);
	mount(api, impatient());
	await expect.element(table()).toBeVisible();
	const failed = table().getByRole("alert");
	await expect
		.poll(async () => {
			bottom();
			return failed.query() !== null;
		})
		.toBe(true);
	await expect.element(failed).toHaveTextContent("These lines did not load");
	down = false;
	await failed.getByRole("button", { name: "Retry" }).click();
	await expect.element(failed).not.toBeInTheDocument();
	expect(
		requests.filter((url) => isReport(url) && pageOf(url) === "2"),
	).toHaveLength(2);
});

// Kills Open N that opens other lines than those chosen, another window, or a window that ends elsewhere.
test("Open N opens exactly the selected lines, over the view's window ending at the report", async () => {
	const report = threeLines({
		window: {
			start_time: Date.parse("2026-09-06T21:18:00Z"),
			end_time: Date.parse("2026-09-13T21:18:00Z"),
			clamped: false,
		},
	});
	const { api } = fakeApi((url) =>
		isReport(url) ? { data: report } : { data: bootstrapOf() },
	);
	mount(api, impatient(), `${PATH}?window=1w`);
	await page.getByRole("checkbox", { name: `Select ${SMALL}` }).click();
	await page.getByRole("checkbox", { name: `Select ${THROUGHPUT}` }).click();
	const open = page.getByRole("link", { name: "Open 2 in Explore" });
	await expect.element(open).toBeVisible();

	const query = decodeQuery(
		new URL(open.element().getAttribute("href") ?? "", location.origin).search,
	);
	const [, small, throughput] = linesOf(report);
	expect([...(query.only ?? [])].sort()).toEqual(
		[small?.key, throughput?.key].sort(),
	);
	expect(query.window).toEqual({
		seconds: 7 * 86_400,
		end: report.window.end_time,
	});
	expect(query.report).toBe(REPORT);
});

// Kills a row whose Explore link opens another line, window, or report than
// Open 1 does for that row alone, and row links that wait for a selection.
test("a row's Open in Explore is Open 1 for that row alone", async () => {
	const { api } = fakeApi((url) =>
		isReport(url) ? { data: threeLines() } : { data: bootstrapOf() },
	);
	mount(api, impatient(), `${PATH}?window=1w`);
	await expect.element(table()).toBeVisible();
	await userEvent.hover(table().getByRole("columnheader", { name: "Line" }));
	await userEvent.hover(table().getByRole("columnheader", { name: "Limit" }));
	const row = page.getByRole("link", { name: `Open in Explore: ${SMALL}` });
	await expect.element(row).toBeVisible();
	const href = row.element().getAttribute("href");
	expect(href).toMatch(/^\/console\/projects\/hashbrown\/explore\?/);
	await page.getByRole("checkbox", { name: `Select ${SMALL}` }).click();
	await expect
		.element(page.getByRole("link", { name: "Open 1 in Explore" }))
		.toHaveAttribute("href", href as string);
});

// Kills a report that keeps its wide rows on a tablet, where the Explore
// column leaves a line's name little room, and none at 768.
test("at 768 and 900 px a report's rows fold so each line's name has room", async () => {
	const { api } = fakeApi((url) =>
		isReport(url) ? { data: threeLines() } : { data: bootstrapOf() },
	);
	for (const width of [768, 900]) {
		await page.viewport(width, 800);
		mount(api, impatient());
		await expect.element(table()).toBeVisible();
		for (const name of [LATENCY, SMALL, THROUGHPUT]) {
			const cell = page
				.getByRole("checkbox", { name: `Select ${name}` })
				.element()
				.closest("tr")
				?.querySelector("b")
				?.closest("td");
			expect(cell?.getBoundingClientRect().width).toBeGreaterThanOrEqual(200);
		}
		dispose?.();
		document.body.replaceChildren();
	}
});

// Kills a Previous or Open N whose name does not start with what it shows,
// wide or narrow, so voice control cannot reach it by its words.
test("Previous and Open N are named first by what they show, at either width", async () => {
	const { api } = fakeApi((url) =>
		isReport(url)
			? {
					data: reportFixture(undefined, { run: run() } as never),
				}
			: { data: bootstrapOf() },
	);
	for (const width of [1280, 390]) {
		await page.viewport(width, 720);
		mount(api, impatient());
		await page.getByRole("checkbox", { name: "Select blake3" }).click();
		const open = page.getByRole("link", { name: "Open 1 in Explore" });
		await expect.element(open).toBeVisible();
		expectNameStartsWithText(open.element());
		expectNameStartsWithText(
			page.getByRole("link", { name: /^Previous / }).element(),
		);
		dispose?.();
		document.body.replaceChildren();
	}
});

// Kills a read-only note that arrives with the bootstrap on its own line and
// pushes the controls and the rows down, at any width.
test("a viewer's read-only note arrives without moving what is below the title", async () => {
	for (const width of [1280, 768, 390]) {
		await page.viewport(width, 800);
		let answer: (bootstrap: unknown) => void = () => {};
		const held = new Promise((resolve) => {
			answer = resolve;
		});
		const { api } = fakeApi(async (url) =>
			isReport(url) ? { data: threeLines() } : { data: await held },
		);
		mount(api, impatient());
		await expect.element(table()).toBeVisible();
		const top = () => table().element().getBoundingClientRect().top;
		const before = top();
		answer(bootstrapOf({ edit: false }));
		await expect
			.element(page.getByText("Read only", { exact: true }))
			.toBeInTheDocument();
		expect(top()).toBe(before);
		dispose?.();
		document.body.replaceChildren();
	}
});

// Kills a Clear that keeps the lines chosen.
test("Clear lets go of every selected line", async () => {
	const { api } = fakeApi((url) =>
		isReport(url) ? { data: threeLines() } : { data: bootstrapOf() },
	);
	mount(api, impatient());
	await page.getByRole("checkbox", { name: `Select ${LATENCY}` }).click();
	await page.getByRole("checkbox", { name: `Select ${SMALL}` }).click();
	await expect.element(page.getByText("2 lines selected")).toBeVisible();
	await page.getByRole("button", { name: "Clear", exact: true }).click();
	await expect.element(page.getByText("0 lines selected")).toBeVisible();
	await expect
		.element(page.getByRole("checkbox", { name: `Select ${LATENCY}` }))
		.not.toBeChecked();
});

// Kills a neighbor prefetch that is missing, asks for another view or batch,
// or sizes the batch for the screen as it is at the hover rather than as the page fixed it.
test("hovering Previous starts its first batch in this view at the page's batch size", async () => {
	const { api, requests } = fakeApi((url) => {
		if (!isReport(url)) {
			return { data: bootstrapOf() };
		}
		return url.pathname.endsWith(PREVIOUS)
			? {
					data: reportFixture(undefined, {
						uuid: PREVIOUS,
						run: run({ order: 4 }),
					} as never),
				}
			: { data: reportFixture(undefined, { run: run() } as never) };
	});
	mount(api, impatient(), `${PATH}?sort=delta`);
	await expect.element(table()).toBeVisible();
	const [first] = requests.filter(isReport);
	await page.viewport(1280, 400);

	await page.getByRole("link", { name: /^Previous #4/ }).hover();
	const previous = () =>
		requests.filter((url) => url.pathname.endsWith(PREVIOUS));
	await expect.poll(() => previous().length).toBe(1);
	const [prefetched] = previous();
	expect(pageOf(prefetched as URL)).toBe("1");
	expect(prefetched?.searchParams.get("per_page")).toBe(
		first?.searchParams.get("per_page"),
	);
	expect(prefetched?.searchParams.get("sort")).toBe("delta");

	await page.getByRole("link", { name: /^Previous #4/ }).click();
	await expect.element(page.getByText(/^#4 of 8 · /)).toBeVisible();
	expect(previous()).toHaveLength(1);
});

// Kills opening a row that adds a step to the browser's history.
test("opening a row replaces the history entry it was opened in", async () => {
	const { api } = fakeApi((url) =>
		isReport(url) ? { data: threeLines() } : { data: bootstrapOf() },
	);
	const history = mount(api, impatient());
	await page.getByRole("radio", { name: "Measure" }).click();
	await page.getByRole("button", { name: `Expand ${LATENCY}` }).click();
	await expect
		.element(page.getByRole("button", { name: `Collapse ${LATENCY}` }))
		.toBeVisible();
	history.back();
	expect(history.get()).toBe(PATH);
});

// Kills a subtitle that counts a filter's matches as the report's lines, and a history heading that ignores the window.
test("the subtitle counts the report's lines unless a filter narrows them, and the history names its window", async () => {
	const { api } = fakeApi((url) =>
		isReport(url) ? { data: threeLines() } : { data: bootstrapOf() },
	);
	mount(api, impatient(), `${PATH}?window=1w`);
	const subtitle = page.getByText(/ · json · /);
	await expect.element(subtitle).toHaveTextContent("3 lines");
	await expect
		.element(page.getByRole("columnheader", { name: "History, 1w" }))
		.toBeVisible();

	dispose?.();
	document.body.replaceChildren();
	mount(api, impatient(), `${PATH}?search=avx2`);
	await expect.element(subtitle).toHaveTextContent("took");
	await expect.element(subtitle).not.toHaveTextContent(/\blines?\b/);
});

// Kills a sheet given none of the report's lines, which guards the default side instead of the project's.
test("the no-threshold sheet guards the side the report's other lines guard", async () => {
	const report = reportFixture(
		[
			lineFixture({ model: 1, upper_limit: undefined, lower_limit: 19 }),
			lineFixture({
				variant: 1,
				model: undefined,
				baseline: undefined,
				upper_limit: undefined,
			}),
		],
		{
			models: [
				{ uuid: "u", threshold: "u", test: "t_test", upper_boundary: 0.99 },
				{ uuid: "l", threshold: "l", test: "t_test", lower_boundary: 0.99 },
			],
		} as never,
	);
	const { api } = fakeApi((url) =>
		isReport(url) ? { data: report } : { data: bootstrapOf() },
	);
	mount(api, impatient());
	await page
		.getByRole("button", {
			name: `No threshold checks ${SMALL}. Show the run snippet that declares one.`,
		})
		.click();
	const exact = page
		.getByRole("dialog", { name: "No threshold checks this line" })
		.getByRole("region", { name: "Exactly this line" });
	await expect
		.element(exact)
		.toHaveTextContent("--threshold-lower-boundary 0.99");
	await expect.element(exact).not.toHaveTextContent("--threshold-upper");
});

// Kills a bound line of a version 0 project offered a threshold it cannot
// declare, a value line's sheet taught the version 1 flags its project refuses
// or an exact threshold on parameters it has none of, and a sheet that names
// `json` rather than the adapter of the line's report.
test("on a version 0 project only a value line offers the no-threshold sheet, with plain flags", async () => {
	const unchecked = {
		model: undefined,
		baseline: undefined,
		upper_limit: undefined,
	};
	const report = reportFixture(
		[
			lineFixture(unchecked),
			lineFixture({ ...unchecked, metric: "lower_value" }),
			lineFixture({ ...unchecked, metric: "upper_value" }),
		],
		{
			adapter: "rust_criterion",
			variants: [{ uuid: "v", benchmark: 0, parameters: {} }],
			counts: {
				benchmarks: 1,
				variants: 1,
				measures: 1,
				metrics: 3,
				iterations: 1,
				alerts: { total: 0, active: 0 },
			},
		} as never,
	);
	const v0 = bootstrapOf();
	const { api } = fakeApi((url) =>
		isReport(url)
			? { data: report }
			: { data: { ...v0, project: { ...v0.project, bmf_version: 0 } } },
	);
	mount(api, impatient());
	await expect.element(table()).toBeVisible();
	await expect
		.poll(() => table().getByText("no threshold", { exact: true }).elements())
		.toHaveLength(3);
	const offers = table().getByRole("button", { name: /^No threshold checks / });
	await expect.poll(() => offers.elements()).toHaveLength(1);
	await offers.click();
	const sheet = page.getByRole("dialog", {
		name: "No threshold checks this line",
	});
	await expect.element(sheet).toHaveTextContent("--adapter rust_criterion");
	await expect.element(sheet).toHaveTextContent("--threshold-measure latency");
	await expect.element(sheet).not.toHaveTextContent("--threshold-metric");
	await expect
		.element(sheet.getByRole("region", { name: "Exactly this line" }))
		.not.toBeInTheDocument();
});

// Kills the version 0 rule applied to a version 1 project, whose lines of any
// metric can have a threshold declared.
test("on a version 1 project a line of any metric offers the no-threshold sheet", async () => {
	const unchecked = {
		model: undefined,
		baseline: undefined,
		upper_limit: undefined,
	};
	const report = reportFixture(
		[
			lineFixture({ ...unchecked, metric: "value" }),
			lineFixture({ ...unchecked, metric: "p99" }),
		],
		{
			counts: {
				benchmarks: 1,
				variants: 2,
				measures: 1,
				metrics: 2,
				iterations: 1,
				alerts: { total: 0, active: 0 },
			},
		} as never,
	);
	const { api } = fakeApi((url) =>
		isReport(url) ? { data: report } : { data: bootstrapOf() },
	);
	mount(api, impatient());
	await expect.element(table()).toBeVisible();
	const offers = table().getByRole("button", { name: /^No threshold checks / });
	await expect.poll(() => offers.elements()).toHaveLength(2);
	await table()
		.getByRole("button", { name: /^No threshold checks .* p99\. / })
		.click();
	await expect
		.element(
			page
				.getByRole("dialog", { name: "No threshold checks this line" })
				.getByRole("region", { name: "Exactly this line" }),
		)
		.toHaveTextContent("--threshold-metric p99");
});

// Kills a host read from anywhere but the project the page is in.
test("the no-threshold sheet names the project's self-hosted API", async () => {
	const { api } = fakeApi((url) =>
		isReport(url) ? { data: threeLines() } : { data: bootstrapOf() },
	);
	mount(api, impatient(), PATH, "http://localhost:61016");
	await page
		.getByRole("button", {
			name: `No threshold checks ${THROUGHPUT}. Show the run snippet that declares one.`,
		})
		.click();
	await expect
		.element(
			page.getByRole("dialog", { name: "No threshold checks this line" }),
		)
		.toHaveTextContent("--host http://localhost:61016");
});

// Kills a Read only pill that the title and its alerts push along when the
// report answers after the bootstrap, as it does on every move to a report
// within the console, at any width.
test("a viewer's Read only pill keeps its place when the report arrives after the bootstrap", async () => {
	for (const width of [1280, 768, 390]) {
		await page.viewport(width, 800);
		let answer: (report: unknown) => void = () => {};
		const held = new Promise((resolve) => {
			answer = resolve;
		});
		const { api } = fakeApi(async (url) =>
			isReport(url)
				? { data: await held }
				: { data: bootstrapOf({ edit: false }) },
		);
		mount(api, impatient());
		const readOnly = page.getByText("Read only", { exact: true });
		await expect.element(readOnly).toBeVisible();
		const pill = () => {
			const { x, y, width, height } = readOnly
				.element()
				.getBoundingClientRect();
			return { x, y, width, height };
		};
		const before = pill();
		answer(threeLines());
		await expect.element(table()).toBeVisible();
		expect(pill()).toEqual(before);
		dispose?.();
		document.body.replaceChildren();
	}
});

// Kills a Read only pill that takes its width from a viewer's title, cutting
// a title a member sees whole, and one that moves the title's alerts when the
// bootstrap answers after the report.
test("a viewer's title stays whole beside the Read only pill, and its alerts keep their place when the bootstrap answers last", async () => {
	for (const width of [1280, 768, 390]) {
		await page.viewport(width, 800);
		let answer: (bootstrap: unknown) => void = () => {};
		const held = new Promise((resolve) => {
			answer = resolve;
		});
		const { api } = fakeApi(async (url) =>
			isReport(url) ? { data: threeLines() } : { data: await held },
		);
		mount(api, impatient());
		const title = page.getByRole("heading", { level: 1 });
		await expect.element(title).toHaveTextContent("main · ubuntu-latest");
		const alerts = page.getByText("1 alert", { exact: true }).first();
		const box = () => {
			const { x, y, width, height } = alerts.element().getBoundingClientRect();
			return { x, y, width, height };
		};
		const before = box();
		answer(bootstrapOf({ edit: false }));
		await expect
			.element(page.getByText("Read only", { exact: true }))
			.toBeVisible();
		expect(box()).toEqual(before);
		const heading = title.element() as HTMLElement;
		expect(heading.scrollWidth).toBeLessThanOrEqual(heading.clientWidth);
		dispose?.();
		document.body.replaceChildren();
	}
});

// Kills a reread whose answer has not changed that redraws an open row's plot
// or the rows, which drops the focus.
test("a reread with the same answer redraws nothing and keeps the focus", async () => {
	const { api, requests } = fakeApi((url) =>
		isReport(url) ? { data: threeLines() } : { data: bootstrapOf() },
	);
	const client = impatient();
	mount(api, client);
	await page.getByRole("button", { name: `Expand ${LATENCY}` }).click();
	const plot = page.getByRole("region", { name: `${LATENCY}, full plot` });
	await expect.element(plot).toBeVisible();
	const drawn = plot.element();
	const select = page.getByRole("checkbox", { name: `Select ${SMALL}` });
	(select.element() as HTMLElement).focus();
	const reads = requests.filter(isReport).length;
	await client.refetchQueries();
	await expect
		.poll(() => requests.filter(isReport).length)
		.toBeGreaterThan(reads);
	await new Promise((resolve) => setTimeout(resolve, 100));
	expect(plot.element()).toBe(drawn);
	expect(document.activeElement).toBe(select.element());
});

// Kills an open row's plot built again whenever another row opens.
test("opening a second row keeps the first row's plot", async () => {
	const { api } = fakeApi((url) =>
		isReport(url) ? { data: threeLines() } : { data: bootstrapOf() },
	);
	mount(api, impatient());
	const plotOf = (name: string) =>
		page.getByRole("region", { name: `${name}, full plot` });
	await page.getByRole("button", { name: `Expand ${LATENCY}` }).click();
	await expect
		.element(plotOf(LATENCY).getByRole("button", { name: /: hide its line$/ }))
		.toBeVisible();
	const canvas = plotOf(LATENCY).element().querySelector("canvas");
	expect(canvas).not.toBeNull();

	await page.getByRole("button", { name: `Expand ${SMALL}` }).click();
	await expect
		.element(plotOf(SMALL).getByRole("button", { name: /: hide its line$/ }))
		.toBeVisible();
	await frame();
	expect(plotOf(LATENCY).element().querySelector("canvas")).toBe(canvas);
});

// Kills a malformed report id sent to the API, which refuses it, instead of shown as not found.
test("a malformed report id is not found at once, without asking the API", async () => {
	const { api, requests } = fakeApi((url) =>
		isReport(url) ? { data: reportFixture() } : { data: bootstrapOf() },
	);
	mount(api, impatient(), `${PROJECTS_PATH}/hashbrown/reports/9c1f2e4`);
	await expect
		.element(page.getByRole("heading", { level: 1 }))
		.toHaveTextContent("Report not found");
	expect(requests.filter(isReport)).toHaveLength(0);
});

const ALERTED = {
	uuid: "alert-uuid",
	limit: "upper",
	status: "active",
} as const;

// The alerted report's one line, as its controls call it.
const ALONE = "blake3";

/** A report of one line that raised `ALERTED`, read as `bootstrap` permits, its changes answered by `change`. */
const alertedReport = (
	bootstrap = bootstrapOf(),
	change: () => Promise<unknown> = async () => ({ changed: 1 }),
) => {
	const sent: { method: string; path: string; body: unknown }[] = [];
	const { api } = fakeApi((url) =>
		isReport(url)
			? {
					data: reportFixture([
						lineFixture({
							alert: ALERTED as never,
							history: { alerts: [{ index: 2, ...ALERTED }] as never },
						}),
					]),
				}
			: { data: bootstrap },
	);
	api.send = async <T,>(method: string, path: string, body?: unknown) => {
		sent.push({ method, path, body });
		return { data: (await change()) as T, headers: new Headers() };
	};
	mount(api, impatient());
	return sent;
};

/** Opens the row's plot and pins its readout on the alert, the last point. */
const pinAlert = async () => {
	await page.getByRole("button", { name: `Expand ${ALONE}` }).click();
	const region = page.getByRole("region", { name: `${ALONE}, full plot` });
	const plot = region.getByRole("group", { name: /^1 line/ });
	await expect.element(plot).toBeVisible();
	(plot.element() as HTMLElement).focus();
	await userEvent.keyboard("{End}");
	const readout = region.getByRole("status");
	await expect
		.element(readout.getByRole("link", { name: `Alert: ${ALONE}` }))
		.toHaveAttribute("href", `${PROJECTS_PATH}/hashbrown/alerts/alert-uuid`);
	return readout;
};

type Scope = ReturnType<typeof page.getByRole>;
const dismissIn = (readout: Scope) =>
	readout.getByRole("button", { name: `Dismiss the alert on ${ALONE}` });
const reactivateIn = (readout: Scope) =>
	readout.getByRole("button", { name: `Reactivate the alert on ${ALONE}` });

// Kills a report row that alerted whose name links nowhere or to another
// alert, a readout on its alert with no link to it or no Dismiss, a Dismiss
// that sends another alert or status, a readout left offering Dismiss, and a
// row plot folded and opened again that forgets the dismissal.
test("a row that alerted links its alert, and its pinned readout dismisses it", async () => {
	const sent = alertedReport();
	await expect
		.element(
			page.getByRole("link", {
				name: "blake3 input_bytes=65536 simd=avx2 threads=1 Latency, open the alert",
			}),
		)
		.toHaveAttribute("href", `${PROJECTS_PATH}/hashbrown/alerts/alert-uuid`);
	const readout = await pinAlert();
	await dismissIn(readout).click();
	await expect.element(reactivateIn(readout)).toBeVisible();
	await expect.poll(() => sent).toHaveLength(1);
	expect(sent[0]).toEqual({
		method: "PATCH",
		path: "/v0/console/projects/hashbrown/alerts",
		body: { status: "dismissed", alerts: ["alert-uuid"] },
	});

	await page.getByRole("button", { name: `Collapse ${ALONE}` }).click();
	await expect.element(readout).not.toBeInTheDocument();
	await expect.element(reactivateIn(await pinAlert())).toBeVisible();
});

// Kills a viewer offered a Dismiss the API refuses.
test("a viewer's pinned readout links the alert and offers no change", async () => {
	alertedReport(bootstrapOf({ edit: false }));
	const readout = await pinAlert();
	expect(
		readout.getByRole("button", { name: /the alert on/ }).elements(),
	).toHaveLength(0);
});

// Kills a refused change from a row's readout that leaves it as if it had changed, or says nothing.
test("a refused Dismiss from a row's readout puts it back, and says so there", async () => {
	alertedReport(bootstrapOf(), () =>
		Promise.reject(new ApiError(500, "server", "{}")),
	);
	const readout = await pinAlert();
	await dismissIn(readout).click();
	await expect.element(readout.getByRole("alert")).toBeVisible();
	await expect.element(dismissIn(readout)).toBeVisible();
});

const crumbs = () => document.querySelector(".crumb-line");
const subLine = () => document.querySelector(".rp-sub")?.textContent ?? "";

// Kills a crumb that leads anywhere but the report's run or names it by
// anything but its branch and head, a sub line that does not lead with the
// report's place in its run, and a hash the sub line repeats beside the crumb's.
test("the crumb leads back to the report's run, and the sub line leads with its place in it", async () => {
	const { api } = fakeApi((url) =>
		isReport(url)
			? { data: reportFixture(undefined, { run: run() } as never) }
			: { data: bootstrapOf() },
	);
	mount(api, impatient());
	const back = page.getByRole("link", { name: "main 9c1f2e4", exact: true });
	await expect
		.element(back)
		.toHaveAttribute("href", `${PROJECTS_PATH}/hashbrown/runs/${RUN}`);
	expect(crumbs()?.textContent).toBe("Runs/main · 9c1f2e4/");
	expect(subLine()).toMatch(
		/^#5 of 8 · Sep 1[34], 2026, \d\d:\d6:00 · json · /,
	);
	expect(subLine()).not.toContain("9c1f2e4");
});

// Kills a report on an archived testbed given a place in its run, and a run
// left out of the runs list still linked, which would open a page not found.
test("a report its run leaves out has no place in it, and one whose run is left out has no run", async () => {
	let answer = reportFixture(undefined, {
		run: run({ order: undefined, total: 7 }),
	} as never);
	const { api } = fakeApi((url) =>
		isReport(url) ? { data: answer } : { data: bootstrapOf() },
	);
	mount(api, impatient());
	await expect
		.element(page.getByRole("link", { name: "main 9c1f2e4", exact: true }))
		.toBeVisible();
	expect(subLine()).toMatch(/^Sep 1[34], 2026/);
	await expect
		.element(page.getByRole("link", { name: /^Previous #4/ }))
		.toBeVisible();
	dispose?.();
	document.body.replaceChildren();

	answer = reportFixture();
	mount(api, impatient());
	await expect.element(table()).toBeVisible();
	expect(crumbs()?.textContent).toBe("Runs/");
	expect(subLine()).toContain("9c1f2e4");
	await expect
		.element(page.getByRole("button", { name: "Previous" }))
		.toBeDisabled();
	await expect
		.element(page.getByRole("button", { name: "Next" }))
		.toBeDisabled();
});

// Kills Previous and Next that lead anywhere but the run's reports beside this
// one, drop the view, name the report by anything but its place and testbed,
// or keep the testbed on a phone where it has no room.
test("Previous and Next walk the run's reports by place and testbed, the testbed left out on a phone", async () => {
	const { api } = fakeApi((url) =>
		isReport(url)
			? { data: reportFixture(undefined, { run: run() } as never) }
			: { data: bootstrapOf() },
	);
	mount(api, impatient(), `${PATH}?sort=delta`);
	const previous = page.getByRole("link", { name: /^Previous / });
	await expect
		.element(previous)
		.toHaveAttribute(
			"href",
			`${PROJECTS_PATH}/hashbrown/reports/${PREVIOUS}?sort=delta`,
		);
	await expect
		.element(previous)
		.toHaveTextContent("Previous #4 · macos-latest");
	expect(previous.element().getAttribute("aria-label")).toMatch(
		/^Previous #4 · macos-latest: report 4 of 8$/,
	);
	const next = page.getByRole("link", { name: /^Next / });
	await expect
		.element(next)
		.toHaveAttribute(
			"href",
			`${PROJECTS_PATH}/hashbrown/reports/${NEXT}?sort=delta`,
		);
	await expect.element(next).toHaveTextContent("Next #6 · macos-latest");

	await page.viewport(390, 844);
	await expect.element(previous).toHaveTextContent(/^Previous #4$/);
	expect(previous.element().getAttribute("aria-label")).toMatch(
		/^Previous #4: report 4 of 8, macos-latest$/,
	);
});

// Kills a move back to the run that is not asked for before the click, or is
// asked for at another batch size than the run's page asks.
test("hovering the run's crumb starts its first batch", async () => {
	const { api, requests } = fakeApi((url) =>
		isReport(url)
			? { data: reportFixture(undefined, { run: run() } as never) }
			: { data: bootstrapOf() },
	);
	mount(api, impatient());
	await page.getByRole("link", { name: "main 9c1f2e4", exact: true }).hover();
	const asked = () => requests.filter((url) => url.pathname.endsWith(RUN));
	await expect.poll(() => asked().length).toBe(1);
	expect(asked()[0]?.pathname).toBe(
		`/v0/console/projects/hashbrown/runs/${RUN}`,
	);
	expect(asked()[0]?.searchParams.get("page")).toBe("1");
	expect(asked()[0]?.searchParams.get("per_page")).toBe(
		String(runScreenBatch()),
	);
});

// Kills a phone sub line that drops the report's place in its run or reads its
// start to the minute, and a report alone in its run read as "#1 of 1".
test("a phone sub line leads with the place, to the second, and a report alone in its run has none", async () => {
	await page.viewport(390, 844);
	let answer = reportFixture(undefined, { run: run() } as never);
	const { api } = fakeApi((url) =>
		isReport(url) ? { data: answer } : { data: bootstrapOf() },
	);
	mount(api, impatient());
	await expect.element(table()).toBeVisible();
	expect(subLine()).toMatch(/^#5 of 8 · Sep 1[34], \d\d:\d6:00 · json/);
	dispose?.();
	document.body.replaceChildren();

	answer = reportFixture(undefined, {
		run: run({ order: 1, total: 1, previous: undefined, next: undefined }),
	} as never);
	mount(api, impatient());
	await expect
		.element(page.getByRole("link", { name: "main 9c1f2e4", exact: true }))
		.toBeVisible();
	expect(subLine()).toMatch(/^Sep 1[34], \d\d:\d6:00 · json/);
});

const STARTED = Date.parse("2026-09-13T21:16:00Z");

/** A runner job's report that has no lines yet, its job in `job`'s state. */
const jobReport = (job: Partial<JsonConsoleJob>) =>
	reportFixture([], {
		total: 0,
		groups: [],
		counts: {
			benchmarks: 0,
			variants: 0,
			measures: 0,
			metrics: 0,
			iterations: 0,
			alerts: { total: 0, active: 0 },
		},
		job: { status: "queued", created: STARTED, ...job } as JsonConsoleJob,
	});

// Kills a running report's page without its status in the heading, a "No
// lines" card while its job still runs, a sub line that counts nothing or
// times a job not yet done, a failed report read as running or timed from its
// submission, and a "No lines" card on a failed or canceled report, whose
// heading already says why it has none.
test("a runner job's report says how its job stands in its heading", async () => {
	vi.useFakeTimers({ toFake: ["Date"] });
	vi.setSystemTime(STARTED + 48_000);
	let report = jobReport({
		status: "running" as JsonConsoleJob["status"],
		started: STARTED,
	});
	const { api } = fakeApi((url) =>
		isReport(url) ? { data: report } : { data: bootstrapOf() },
	);
	const heading = () => document.querySelector(".rp-titlerow");
	const none = page.getByRole("heading", { name: "No lines", exact: true });
	mount(api, impatient());
	await expect.element(page.getByText("Running 48s")).toBeVisible();
	expect(heading()?.textContent).toContain("Running 48s");
	expect(none.elements()).toHaveLength(0);
	expect(subLine()).not.toMatch(/benchmark|took/);

	for (const [ended, shown] of [
		[
			{
				status: "failed",
				started: STARTED + 10_000,
				completed: STARTED + 52_000,
			},
			"Failed 42s",
		],
		[{ status: "canceled", completed: STARTED + 30_000 }, "Canceled"],
	] as const) {
		dispose?.();
		document.body.replaceChildren();
		report = jobReport(ended as Partial<JsonConsoleJob>);
		mount(api, impatient());
		await expect.element(page.getByText(shown, { exact: true })).toBeVisible();
		expect(heading()?.textContent).toContain(shown);
		expect(none.elements()).toHaveLength(0);
	}
});

// Kills a report page whose running time stands still as its job's answers come in.
test("a running report's time moves on with each answer", async () => {
	vi.useFakeTimers({ toFake: ["Date"] });
	vi.setSystemTime(STARTED + 48_000);
	const { api } = fakeApi((url) =>
		isReport(url)
			? {
					data: jobReport({
						status: "running" as JsonConsoleJob["status"],
						started: STARTED,
					}),
				}
			: { data: bootstrapOf() },
	);
	const client = impatient();
	mount(api, client);
	await expect.element(page.getByText("Running 48s")).toBeVisible();
	vi.setSystemTime(STARTED + 3 * 60_000);
	await client.refetchQueries();
	await expect.element(page.getByText("Running 3m")).toBeVisible();
});

const spec = {
	name: "Standard x86",
	slug: "standard-x86",
	os: "linux",
	architecture: "x86_64",
	cpu: 8,
	memory: 16 * 1024 ** 3,
	disk: 100 * 1024 ** 3,
} as JsonConsoleSpec;

// Kills a runner report without its spec in the sub line, or with what it ran
// on only in a tooltip, which a phone never shows.
test("a runner report's sub line names its spec and what it ran on, a reveal where it is cut", async () => {
	const long = { ...spec, name: `Standard ${"x".repeat(56)}` };
	for (const [width, shown, cut] of [
		[1280, spec, false],
		[390, long, true],
	] as const) {
		await page.viewport(width, 844);
		const { api } = fakeApi((url) =>
			isReport(url)
				? { data: reportFixture(undefined, { spec: shown }) }
				: { data: bootstrapOf() },
		);
		mount(api, impatient());
		const text = `${shown.name} (linux, x86_64, 8, 16 GiB)`;
		await expect
			.element(page.getByText(text, { exact: true }))
			.toBeInTheDocument();
		const reveal = page.getByRole("button", { name: text });
		if (cut) {
			await reveal.click();
			await expect.element(reveal).toHaveAttribute("aria-expanded", "true");
		} else {
			expect(reveal.elements()).toHaveLength(0);
		}
		expect(page.getByRole("tooltip").elements()).toHaveLength(0);
		dispose?.();
		document.body.replaceChildren();
	}
});

// Kills a phone head that grows when the report arrives, and a sub line that
// cuts a part rather than wrapping it whole or scrolls the page sideways.
test("on a phone the sub line wraps its parts whole in the two lines held for them", async () => {
	for (const width of [320, 390]) {
		await page.viewport(width, 844);
		const answer = Promise.withResolvers<JsonConsoleReport>();
		const { api } = fakeApi((url) =>
			isReport(url)
				? answer.promise.then((data) => ({ data }))
				: { data: bootstrapOf() },
		);
		mount(api, impatient());
		const sub = () => document.querySelector<HTMLElement>(".rp-sub");
		await expect
			.poll(() => sub()?.querySelector(".ui-skeleton"))
			.not.toBeNull();
		const held = sub()?.getBoundingClientRect().height;
		answer.resolve(
			reportFixture(undefined, {
				spec,
				run: { ...run(), order: 3, total: 8 },
			} as never),
		);
		await expect.poll(() => sub()?.querySelector(".ui-skeleton")).toBeNull();
		expect(sub()?.getBoundingClientRect().height).toBe(held);
		for (const part of sub()?.querySelectorAll<HTMLElement>("[data-fixed]") ??
			[]) {
			expect(part.scrollWidth, part.textContent ?? "").toBeLessThanOrEqual(
				part.clientWidth,
			);
		}
		expect(document.documentElement.scrollWidth).toBe(width);
		dispose?.();
		document.body.replaceChildren();
	}
});

// Kills a hash Copy that copies the short hash or nothing, changes its width on
// a press, or never says it copied.
test("Copy beside a report's hash copies the whole hash", async () => {
	const writes = vi
		.spyOn(navigator.clipboard, "writeText")
		.mockResolvedValue(undefined);
	const { api } = fakeApi((url) =>
		isReport(url) ? { data: reportFixture() } : { data: bootstrapOf() },
	);
	mount(api, impatient());
	const copy = page.getByRole("button", { name: "Copy hash" });
	await expect.element(copy).toBeVisible();
	const width = copy.element().getBoundingClientRect().width;
	await copy.click();
	expect(writes).toHaveBeenCalledWith(`9c1f2e4${"0".repeat(33)}`);
	const copied = page.getByRole("button", { name: "Copied hash" });
	await expect.element(copied).toBeVisible();
	expect(copied.element().getBoundingClientRect().width).toBe(width);
	// Both words hold one place, so neither press moves the head.
	const [copyWord, copiedWord] = [".rn-copyto", ".rn-copied"].map(
		(word) =>
			copied.element().querySelector(word)?.getBoundingClientRect() ?? null,
	);
	expect(copyWord?.top).toBe(copiedWord?.top);
	expect(copyWord?.left).toBe(copiedWord?.left);

	await expect
		.element(page.getByRole("status").filter({ hasText: "Copied the hash" }))
		.toBeInTheDocument();
});

// Kills a title that gives its testbed less than a long branch's share of the
// line, cuts it where it fits, or lets either push the page sideways, and a
// crumb that breaks its own words beside the run's long link.
test("a long branch shares the title with its testbed, whole where it fits", async () => {
	for (const width of [320, 1280]) {
		await page.viewport(width, 844);
		const { api } = fakeApi((url) =>
			isReport(url)
				? {
						data: reportFixture(undefined, {
							branch: {
								uuid: "branch",
								name: STRESS.branch,
								slug: "b",
								head: "head",
							},
							run: run(),
						} as never),
					}
				: { data: bootstrapOf() },
		);
		mount(api, impatient());
		const heading = page.getByRole("heading", { level: 1 });
		await expect.element(heading).toHaveTextContent(STRESS.branch);
		const [branch, testbed] = heading
			.element()
			.querySelectorAll<HTMLElement>(".ui-trunc");
		expect(branch?.dataset.tail).toBe("very-long-leaf");
		if (width > 320) {
			expect(testbed?.dataset.cut).toBeUndefined();
		} else {
			expect(testbed?.getBoundingClientRect().width).toBeGreaterThanOrEqual(
				(branch?.getBoundingClientRect().width ?? Number.NaN) - 0.5,
			);
		}
		expect(document.documentElement.scrollWidth).toBe(width);
		// The crumb's words stay whole on one line beside the run's long link.
		const runs = page
			.getByRole("link", { name: "Runs", exact: true })
			.element();
		const words = document.createRange();
		words.selectNodeContents(runs);
		expect(words.getClientRects()).toHaveLength(1);
		dispose?.();
		document.body.replaceChildren();
	}
});
