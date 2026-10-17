import { describe, expect, test } from "vitest";
import {
	AlertStatus,
	BoundaryLimit,
	type JsonAlert,
	type JsonConsoleReport,
	type JsonReport,
} from "../../types/bencher";
import { DEFAULT_WINDOW, MAX_ENTRIES, blankQuery } from "../query/query";
import {
	alertLine,
	benchmarkOptions,
	reportBenchmarkQuery,
	reportBenchmarks,
} from "./start";

const uuid = (n: number) =>
	`00000000-0000-4000-8000-${n.toString(16).padStart(12, "0")}`;

const named = (n: number, name: string) => ({
	uuid: uuid(n),
	project: uuid(99),
	name,
	slug: name,
	created: "2026-09-01T00:00:00Z",
	modified: "2026-09-01T00:00:00Z",
});

const ALERT = {
	uuid: uuid(60),
	report: uuid(61),
	iteration: 0,
	benchmark: named(5, "blake3"),
	variant: {
		uuid: uuid(30),
		benchmark: uuid(5),
		parameters: { size: 64 },
		created: "2026-09-01T00:00:00Z",
		modified: "2026-09-01T00:00:00Z",
	},
	value: 20.6,
	threshold: {
		uuid: uuid(40),
		project: uuid(99),
		branch: { ...named(1, "main"), head: { uuid: uuid(11) } },
		testbed: named(3, "linux"),
		measure: { ...named(7, "Latency"), units: "ns" },
		created: "2026-09-01T00:00:00Z",
		modified: "2026-09-01T00:00:00Z",
	},
	boundary: { baseline: 18.1, upper_limit: 19.4 },
	limit: BoundaryLimit.Upper,
	status: AlertStatus.Active,
	created: "2026-09-13T21:16:00Z",
	modified: "2026-09-13T21:16:00Z",
} as unknown as JsonAlert;

describe("alertLine", () => {
	// Kills reading a dimension from the wrong part of the alert, or a metric other than the threshold's.
	test("name the line an alert fired on", () => {
		expect(alertLine(ALERT)).toEqual({
			branch: { uuid: uuid(1) },
			testbed: { uuid: uuid(3) },
			benchmark: uuid(5),
			parameters: { size: 64 },
			measure: uuid(7),
			metric: "value",
			alerting: true,
		});
		expect(
			alertLine({
				...ALERT,
				threshold: { ...ALERT.threshold, metric: "p99" },
			}).metric,
		).toBe("p99");
	});
});

const measure = (n: number, name: string) => ({
	uuid: uuid(n),
	name,
	slug: name.toLowerCase(),
	units: "ns",
});

// The first page of a report's lines: one line, and every benchmark's group.
const PAGE = {
	groups: [
		{ key: 1, lines: 4, variants: 2, alerts: 0, measures: [1, 0] },
		{ key: 0, lines: 1, variants: 1, alerts: 0, measures: [0] },
	],
	benchmarks: [named(5, "blake3"), named(6, "sha256")],
	measures: [measure(7, "Latency"), measure(8, "Throughput")],
} as unknown as JsonConsoleReport;

describe("reportBenchmarks", () => {
	// Kills benchmarks taken from the page's one line rather than its groups,
	// out of drawing order, or given another group's measures.
	test("name each group's benchmark with its own measures, in drawing order", () => {
		expect(
			reportBenchmarks(PAGE).map(({ name, measures }) => [
				name,
				measures.map((each) => each.name),
			]),
		).toEqual([
			["sha256", ["Throughput", "Latency"]],
			["blake3", ["Latency"]],
		]);
	});
});

const NOW = Date.UTC(2026, 9, 16);
const DAY = 24 * 60 * 60 * 1000;

/** A report that ran `days` before now, for a minute. */
const reportAt = (days: number) =>
	({
		uuid: uuid(61),
		branch: { ...named(1, "main"), head: { uuid: uuid(11) } },
		testbed: named(3, "linux"),
		start_time: new Date(NOW - days * DAY).toISOString(),
		end_time: new Date(NOW - days * DAY + 60_000).toISOString(),
	}) as unknown as JsonReport;
const REPORT = reportAt(1);

describe("reportBenchmarkQuery", () => {
	// Kills a pick that draws another head than the report's, narrows the
	// benchmark to some variants, leaves out a measure it reported, reads past
	// the boxes' limit, keeps a window other than the default, or forgets the
	// report it came from.
	test("draw every variant of the benchmark on the report's head and testbed, in its measures", () => {
		const [sha256] = reportBenchmarks(PAGE);
		expect(sha256 && reportBenchmarkQuery(REPORT, sha256, NOW)).toEqual({
			...blankQuery(),
			branches: [{ uuid: uuid(1), head: uuid(11) }],
			testbeds: [{ uuid: uuid(3) }],
			benchmarks: [uuid(6)],
			measures: [uuid(8), uuid(7)],
			report: uuid(61),
		});
		const many = Array.from({ length: MAX_ENTRIES + 1 }, (_, n) =>
			measure(20 + n, `m${n}`),
		);
		expect(
			reportBenchmarkQuery(
				REPORT,
				{ uuid: uuid(6), name: "sha256", measures: many },
				NOW,
			).measures,
		).toEqual(many.slice(0, MAX_ENTRIES).map((each) => each.uuid));
	});

	// Kills a report older than the default window drawn over a window that
	// holds none of its points, and one inside it drawn over anything else.
	test("draw a report older than the default window over the window as long that ends at it", () => {
		const [sha256] = reportBenchmarks(PAGE);
		const old = reportAt(35);
		expect(sha256 && reportBenchmarkQuery(old, sha256, NOW).window).toEqual({
			seconds: DEFAULT_WINDOW,
			end: Date.parse(old.end_time),
		});
		expect(
			sha256 && reportBenchmarkQuery(reportAt(27), sha256, NOW).window,
		).toEqual(blankQuery().window);
	});
});

describe("benchmarkOptions", () => {
	// Kills a search that heeds case, misses a name holding the text past its
	// start, or lists more than a menu holds.
	test("narrow by any part of the name, ignoring case, to at most a menu's worth", () => {
		const benchmarks = ["blake3", "sha256", "SHA512", "xxh3"].map(
			(name, n) => ({
				uuid: uuid(n),
				name,
				measures: [],
			}),
		);
		expect(
			benchmarkOptions(benchmarks, "sha", 32).map(({ label }) => label),
		).toEqual(["sha256", "SHA512"]);
		expect(
			benchmarkOptions(benchmarks, "3", 32).map(({ label }) => label),
		).toEqual(["blake3", "xxh3"]);
		expect(benchmarkOptions(benchmarks, "", 2)).toHaveLength(2);
	});
});
