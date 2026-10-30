import { describe, expect, test, vi } from "vitest";
import {
	AlertStatus,
	BoundaryLimit,
	type JsonAlert,
	type JsonConsoleReport,
} from "../../types/bencher";
import type { Api } from "../api";
import { DEFAULT_WINDOW, MAX_ENTRIES, blankQuery } from "../query/query";
import {
	type RunReport,
	START,
	alertLine,
	benchmarkOptions,
	reportBenchmarkQuery,
	reportBenchmarks,
	runTime,
	startQueries,
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

/** A report of a run that ran `days` before now, for a minute. */
const reportAt = (days: number): RunReport => ({
	uuid: uuid(61),
	branch: { uuid: uuid(1), head: uuid(11) },
	testbed: named(3, "linux"),
	start_time: NOW - days * DAY,
	end_time: NOW - days * DAY + 60_000,
});
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
	// holds none of its points, one judged by its end rather than its start, and
	// one inside it drawn over anything else.
	test("draw a report older than the default window over the window as long that ends at it", () => {
		const [sha256] = reportBenchmarks(PAGE);
		const old = reportAt(35);
		expect(sha256 && reportBenchmarkQuery(old, sha256, NOW).window).toEqual({
			seconds: DEFAULT_WINDOW,
			end: old.end_time,
		});
		expect(
			sha256 && reportBenchmarkQuery(reportAt(27), sha256, NOW).window,
		).toEqual(blankQuery().window);
		// It started half a minute before the window and ended inside it.
		const straddling = reportAt(28 + 0.5 / (24 * 60));
		expect(
			sha256 && reportBenchmarkQuery(straddling, sha256, NOW).window,
		).toEqual({ seconds: DEFAULT_WINDOW, end: straddling.end_time });
	});
});

describe("benchmarkOptions", () => {
	// Kills a search that heeds case or misses a name holding the text past its start.
	test("narrow by any part of the name, ignoring case", () => {
		const benchmarks = ["blake3", "sha256", "SHA512", "xxh3"].map(
			(name, n) => ({
				uuid: uuid(n),
				name,
				measures: [],
			}),
		);
		expect(
			benchmarkOptions(benchmarks, "sha").map(({ label }) => label),
		).toEqual(["sha256", "SHA512"]);
		expect(benchmarkOptions(benchmarks, "3").map(({ label }) => label)).toEqual(
			["blake3", "xxh3"],
		);
	});

	// Kills the benchmark named what is typed left where the list put it, or
	// found by a match that heeds case.
	test("put the benchmark named what is typed first, ignoring case", () => {
		const benchmarks = ["sha256", "sha512", "Sha"].map((name, n) => ({
			uuid: uuid(n),
			name,
			measures: [],
		}));
		expect(
			benchmarkOptions(benchmarks, "SHA").map(({ label }) => label),
		).toEqual(["Sha", "sha256", "sha512"]);
	});
});

describe("runTime", () => {
	// Kills a run's time read in the wrong unit at either boundary.
	test("read seconds, then minutes and seconds, then hours and minutes", () => {
		expect([59_999, 60_000, 3_599_999, 3_600_000].map(runTime)).toEqual([
			"59s",
			"1m 00s",
			"59m 59s",
			"1h 00m",
		]);
	});
});

describe("startQueries", () => {
	const RECENT = { uuid: uuid(70) };
	const OLD = { uuid: uuid(71) };
	/** The runs list as the API answers it: `recent` inside the window, `older` before it. */
	const listing = (recent: unknown[], older: unknown[]) => {
		const asked: string[] = [];
		const api: Api = {
			get: async <T>(path: string) => {
				asked.push(path);
				const runs = path.includes("start_time=")
					? recent
					: [...recent, ...older];
				return {
					data: { total: runs.length, runs } as T,
					headers: new Headers(),
				};
			},
			send: async () => {
				throw new Error("The start lists send no changes");
			},
		};
		return { api, asked };
	};

	// Kills the latest runs asked for over all time first, which groups every
	// run of a project on blank Explore's cold paint, or over another window
	// than the Runs tab's default.
	test("ask for the latest runs inside the Runs tab's default window", async () => {
		vi.useFakeTimers({ toFake: ["Date"], now: NOW });
		try {
			const { api, asked } = listing([RECENT], [OLD]);
			expect(await startQueries(api, "hashbrown").runs.queryFn()).toEqual([
				RECENT,
			]);
			expect(asked).toEqual([
				`/v0/console/projects/hashbrown/runs?per_page=${START.runs}&start_time=${NOW - DEFAULT_WINDOW * 1000}`,
			]);
		} finally {
			vi.useRealTimers();
		}
	});

	// Kills a project whose runs all predate the window read as one that never
	// ran, which shows it the setup card.
	test("ask again over all time when nothing ran inside the window", async () => {
		const { api, asked } = listing([], [OLD]);
		expect(await startQueries(api, "hashbrown").runs.queryFn()).toEqual([OLD]);
		expect(asked).toHaveLength(2);
		expect(asked[1]).toBe(
			`/v0/console/projects/hashbrown/runs?per_page=${START.runs}`,
		);
	});
});
