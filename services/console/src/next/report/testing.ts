import type {
	JsonConsoleReport,
	JsonConsoleReportLine,
} from "../../types/bencher";

export const REPORT = "00000000-0000-4000-8000-000000000001";
const START = Date.parse("2026-09-13T21:16:00Z");
const DAY = 86_400_000;

type Line = {
	[K in keyof Omit<JsonConsoleReportLine, "history">]?:
		| JsonConsoleReportLine[K]
		| undefined;
} & { history?: Partial<JsonConsoleReportLine["history"]> };

/** A line of blake3 at 64 KiB, its latency checked by an upper threshold. */
export const lineFixture = (fields: Partial<Line> = {}): Line => ({
	benchmark: 0,
	variant: 0,
	measure: 0,
	metric: "value",
	value: 20.6,
	model: 0,
	baseline: 19.4,
	upper_limit: 19.9,
	...fields,
});

/**
 * One batch of a report's lines as the report endpoint returns it, over three
 * points, with two variants of blake3, latency and throughput, and one model.
 */
export const reportFixture = (
	lines: Line[] = [lineFixture()],
	fields: Partial<JsonConsoleReport> = {},
): JsonConsoleReport =>
	({
		uuid: REPORT,
		branch: { uuid: "branch", name: "main", slug: "main", head: "head" },
		testbed: { uuid: "testbed", name: "ubuntu-latest", slug: "ubuntu-latest" },
		version: { number: 24, hash: `9c1f2e4${"0".repeat(33)}` },
		start_time: START,
		end_time: START + 120_000,
		adapter: "json",
		counts: {
			benchmarks: 1,
			variants: 2,
			measures: 2,
			metrics: 1,
			alerts: { total: 1, active: 1 },
		},
		window: { start_time: START - 28 * DAY, end_time: START, clamped: false },
		total: lines.length,
		groups: [{ key: 0, lines: lines.length, variants: 2, alerts: 1 }],
		lines: lines.map(({ history, ...line }) => ({
			...line,
			history: { y: [19.3, 19.5, line.value], alerts: [], ...history },
		})),
		points: { x: [START - 2 * DAY, START - DAY, START], report: [0, 1, 2] },
		reports: [
			{ uuid: "r0", version: 22, hash: "aaaaaaa" },
			{ uuid: "r1", version: 23, hash: "bbbbbbb" },
			{ uuid: REPORT, version: 24, hash: "9c1f2e4" },
		],
		benchmarks: [{ uuid: "blake3-uuid", name: "blake3", slug: "blake3" }],
		variants: [
			{
				uuid: "v64",
				benchmark: 0,
				parameters: { input_bytes: 65536, simd: "avx2", threads: 1 },
			},
			{
				uuid: "v1",
				benchmark: 0,
				parameters: { input_bytes: 1024, simd: "avx2", threads: 1 },
			},
		],
		measures: [
			{
				uuid: "latency-uuid",
				name: "Latency",
				slug: "latency",
				units: "nanoseconds (ns)",
			},
			{
				uuid: "throughput-uuid",
				name: "Throughput",
				slug: "throughput",
				units: "bytes per nanosecond",
			},
		],
		models: [
			{
				uuid: "model",
				threshold: "threshold",
				test: "t_test",
				upper_boundary: 0.99,
			},
		],
		...fields,
	}) as unknown as JsonConsoleReport;

/** A report of `count` lines, a variant each, under benchmarks of ten. */
export const longReport = (count: number): JsonConsoleReport => {
	const report = reportFixture(
		Array.from({ length: count }, (_, index) =>
			lineFixture({ variant: index, benchmark: Math.floor(index / 10) }),
		),
	);
	return {
		...report,
		benchmarks: Array.from({ length: Math.ceil(count / 10) }, (_, index) => ({
			uuid: `b${index}`,
			name: `bench-${index}`,
			slug: `bench-${index}`,
		})),
		variants: Array.from({ length: count }, (_, index) => ({
			uuid: `v${index}`,
			benchmark: Math.floor(index / 10),
			parameters: { n: index % 10 },
		})),
		groups: Array.from({ length: Math.ceil(count / 10) }, (_, index) => ({
			key: index,
			lines: Math.min(10, count - index * 10),
			variants: Math.min(10, count - index * 10),
			alerts: 0,
		})),
	};
};

/** The batch of `report` that a request names by its page and size. */
export const batchOf = (report: JsonConsoleReport, url: URL) => {
	const page = Number(url.searchParams.get("page") ?? 1);
	const size = Number(url.searchParams.get("per_page") ?? report.lines.length);
	return {
		...report,
		lines: report.lines.slice((page - 1) * size, page * size),
	};
};
