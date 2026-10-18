import {
	Adapter,
	type JsonConsoleRun,
	type JsonConsoleRunJobs,
	type JsonConsoleRunReport,
	type JsonConsoleRunSummary,
} from "../../types/bencher";

const START = Date.parse("2026-09-13T21:16:00Z");
const HOUR = 60 * 60 * 1_000;

const uuid = (index: number) =>
	`00000000-0000-4000-8000-${String(index).padStart(12, "0")}`;

const branch = {
	uuid: "branch",
	name: "main",
	slug: "main",
	head: "head",
} as JsonConsoleRunSummary["branch"];

/** A run as the list endpoint returns it, the `index`th newest. */
export const runFixture = (
	index: number,
	fields: Fields<JsonConsoleRunSummary> = {},
): JsonConsoleRunSummary =>
	({
		uuid: uuid(index),
		number: 100 - index,
		hash: `9c1f2e4${"0".repeat(33)}`,
		branch,
		start_time: START - index * HOUR,
		end_time: START - index * HOUR + 120_000,
		reports: 1,
		testbeds: ["ubuntu-latest"],
		testbed_count: 1,
		adapters: ["json"],
		adapter_count: 1,
		authors: [{ kind: "user", name: "Muriel Bagge" }],
		author_count: 1,
		lines: 36,
		alerts: { total: 0, active: 0 },
		...fields,
	}) as JsonConsoleRunSummary;

/** A report of a run, the `index`th by start. */
export const runReportFixture = (
	index: number,
	fields: Fields<JsonConsoleRunReport> = {},
): JsonConsoleRunReport =>
	({
		order: index + 1,
		uuid: uuid(1_000 + index),
		testbed: { uuid: "testbed", name: "ubuntu-latest", slug: "ubuntu-latest" },
		adapter: "json",
		start_time: START + index * 1_000,
		end_time: START + index * 1_000 + 120_000,
		lines: 36,
		alerts: { total: 0, active: 0 },
		author: { kind: "user", name: "Muriel Bagge" },
		...fields,
	}) as JsonConsoleRunReport;

/** A run's page as the run endpoint returns it, with `reports` as its page of reports. */
export const runPageFixture = (
	reports: JsonConsoleRunReport[],
	fields: Fields<JsonConsoleRun> = {},
): JsonConsoleRun =>
	({
		uuid: uuid(0),
		number: 248,
		hash: `9c1f2e4${"0".repeat(33)}`,
		branch,
		start_time: START,
		end_time: START + 848_000,
		total: reports.length,
		testbeds: 1,
		lines: 36 * reports.length,
		alerts: { total: 0, active: 0 },
		reports,
		...fields,
	}) as JsonConsoleRun;

/** Some of a response's fields, an optional one left out by `undefined`. */
type Fields<T> = { [K in keyof T]?: T[K] | undefined };

/** A run as wide as a likely one reads: a long branch, adapter, and author, and three-digit alerts. */
export const likelyRun = (
	index: number,
	fields: Fields<JsonConsoleRunSummary> = {},
) =>
	runFixture(index, {
		branch: { ...branch, name: "feature-simd", slug: "feature-simd" },
		adapters: [Adapter.RustCriterion],
		alerts: { total: 456, active: 123 },
		...fields,
	});

/** Likely runs at `now`: one finished 11 months ago, the widest time a row shows, then the longest Running and Failed pills. */
export const likelyRuns = (now: number) => {
	const jobs = (fields: Partial<JsonConsoleRunJobs>) =>
		({
			queued: 0,
			running: 0,
			failed: 0,
			canceled: 0,
			...fields,
		}) as JsonConsoleRunJobs;
	const start_time = now - 30 * HOUR;
	const failed = (index: number, took: number) =>
		likelyRun(index, {
			reports: 2,
			start_time,
			end_time: start_time + took,
			jobs: jobs({ failed: 2 }),
		});
	const old = now - 335 * 24 * HOUR;
	return [
		likelyRun(0, { start_time: old, end_time: old + 120_000 }),
		likelyRun(1, {
			reports: 2,
			jobs: jobs({ running: 2, since: now - 59 * 60_000 }),
		}),
		failed(2, 59 * 60_000 + 59_000),
		failed(3, 25 * HOUR + 59 * 60_000),
	];
};

/** A run's report as wide as a likely one reads: a long adapter and three-digit alerts. */
export const likelyReport = (
	index: number,
	fields: Fields<JsonConsoleRunReport> = {},
) =>
	runReportFixture(index, {
		adapter: Adapter.RustCriterion,
		alerts: { total: 456, active: 123 },
		...fields,
	});

/** How far `cell`'s content, or anything in it, runs past its box. */
export const overrun = (cell: Element) =>
	Math.max(
		0,
		...[cell, ...cell.querySelectorAll("*")].map(
			(element) => element.scrollWidth - element.clientWidth,
		),
	);

/** The text of each of `row`'s cells, in `columns` (by header) if given, that the reader sees cut off. */
export const cutCells = (row: Element, columns?: readonly string[]) => {
	const headers = [
		...(row.closest("table")?.querySelectorAll("thead th") ?? []),
	].map((th) => th.textContent?.trim() ?? "");
	return [...row.querySelectorAll("td")]
		.filter(
			(cell, index) =>
				(!columns || columns.includes(headers[index] ?? "")) &&
				overrun(cell) > 0.5,
		)
		.map((cell) => `${headers[cell.cellIndex]}: ${cell.textContent?.trim()}`);
};

/** Each header of `table` whose text and padding are wider than its box, which a header's own scrollWidth never shows. */
export const cutHeaders = (table: Element) =>
	[...table.querySelectorAll<HTMLElement>("thead th")]
		.filter((th) => {
			const text = document.createRange();
			text.selectNodeContents(th);
			const style = getComputedStyle(th);
			return (
				text.getBoundingClientRect().width +
					Number.parseFloat(style.paddingLeft) +
					Number.parseFloat(style.paddingRight) >
				th.getBoundingClientRect().width + 0.5
			);
		})
		.map((th) => th.textContent?.trim());
