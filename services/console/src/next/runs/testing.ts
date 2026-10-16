import type {
	JsonConsoleRun,
	JsonConsoleRunReport,
	JsonConsoleRunSummary,
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
