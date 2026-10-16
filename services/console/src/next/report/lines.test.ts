import { describe, expect, test } from "vitest";
import { lineKey } from "../query/line";
import { alignSeries, groupsOf, lineAt, linesOf, plotOf } from "./lines";
import { REPORT, lineFixture, reportFixture } from "./testing";

// The report ran three times, after two run once; its second iteration alerted, so its row shows that one.
const RUNS = reportFixture(
	[
		lineFixture({
			value: 20.6,
			iteration: 1,
			iterations: 3,
			baseline: 19.4,
			upper_limit: 19.9,
			alert: { uuid: "alert", limit: "upper", status: "active" } as never,
			history: {
				y: [19.3, 19.5, 19.6, 20.6, 19.7],
				baseline: [null, 19.3, 19.4, 19.4, 19.5],
				upper: [null, 19.8, 19.9, 19.9, 20],
				alerts: [{ index: 3, uuid: "alert", limit: "upper", status: "active" }],
			} as never,
		}),
	],
	{
		points: {
			x: [1, 2, 3, 3, 3],
			report: [0, 1, 2, 2, 2],
			iteration: [0, 0, 0, 1, 2],
		},
	},
);

describe("linesOf", () => {
	// Kills resolving a line's indexes against the wrong table, or dropping the variant's parameters.
	test("names each line from its batch's tables", () => {
		const batch = reportFixture([
			lineFixture({ variant: 1, measure: 1, model: undefined }),
		]);
		const [line] = linesOf(batch);
		expect(line).toMatchObject({
			benchmark: { name: "blake3" },
			parameters: { input_bytes: 1024, simd: "avx2", threads: 1 },
			measure: { name: "Throughput", units: "bytes per nanosecond" },
			metric: "value",
			value: 20.6,
		});
	});

	// Kills a guard read from anything but the model that checked the value.
	test("guards the side its model bounds, and nothing without a model", () => {
		const batch = reportFixture([
			lineFixture(),
			lineFixture({ variant: 1, model: undefined }),
		]);
		expect(linesOf(batch).map(({ guard }) => guard)).toEqual([
			"upper",
			undefined,
		]);
	});

	// Kills the iteration a row shows, its count, or its points' iterations lost on the way to the row.
	test("keeps the iteration each row shows and its points' iterations", () => {
		const [line] = linesOf(RUNS);
		expect(line).toMatchObject({ iteration: 1, iterations: 3 });
		expect(line?.points.iteration).toEqual([0, 0, 0, 1, 2]);
		expect(plotOf(line as never, "main", "ubuntu-latest").iteration).toEqual([
			0, 0, 0, 1, 2,
		]);
	});

	// Kills a key Explore would not give the same line, so an open row or a selection would name nothing there.
	test("keys each line as Explore does", () => {
		const [line] = linesOf(reportFixture());
		expect(line?.key).toBe(
			lineKey({
				branch: "branch",
				testbed: "testbed",
				benchmark: "blake3-uuid",
				parameters: { input_bytes: 65536, simd: "avx2", threads: 1 },
				measure: "latency-uuid",
				metric: "value",
			}),
		);
	});
});

describe("lineAt", () => {
	// Kills a row that keeps the iteration it showed, reads another report's point of the same iteration, counts from 0, or keeps the alert of the iteration it showed.
	test("reads the chosen iteration of the report from the line's history", () => {
		const [line] = linesOf(RUNS);
		if (!line) {
			throw new Error("The batch has a line");
		}
		expect(lineAt(line, REPORT, 1)).toMatchObject({
			value: 19.6,
			baseline: 19.4,
			upper_limit: 19.9,
			alert: undefined,
			iteration: 0,
			chosen: 1,
		});
		expect(lineAt(line, REPORT, 3)).toMatchObject({
			value: 19.7,
			baseline: 19.5,
			upper_limit: 20,
			iteration: 2,
			chosen: 3,
		});
		expect(lineAt(line, REPORT, 2)).toMatchObject({
			value: 20.6,
			alert: { uuid: "alert", limit: "upper", status: "active" },
			chosen: 2,
		});
	});

	// Kills a row emptied by an iteration its line never ran, or by no choice at all.
	test("keeps the line when nothing is chosen or it has no such iteration", () => {
		const [line] = linesOf(RUNS);
		if (!line) {
			throw new Error("The batch has a line");
		}
		expect(lineAt(line, REPORT, undefined)).toBe(line);
		expect(lineAt(line, REPORT, 4)).toBe(line);
	});
});

describe("alignSeries", () => {
	// Kills drawing a thinned history's values, or the alerts behind its markers, at their own positions instead of the page's points.
	test("spreads a thinned history over the page's points", () => {
		expect(
			alignSeries(
				{
					index: [0, 3],
					y: [1, 4],
					upper: [2, 5],
					alerts: [{ index: 1, uuid: "a", limit: "upper", status: "active" }],
				} as never,
				5,
			),
		).toEqual({
			y: [1, null, null, 4, null],
			upper: [2, null, null, 5, null],
			alerts: [3],
			raised: [{ index: 3, uuid: "a", status: "active", limit: "upper" }],
		});
	});

	// Kills realigning a history that already is.
	test("keeps an aligned history as it is", () => {
		const series = {
			y: [1, null, 3],
			baseline: [1, 1, 1],
			alerts: [{ index: 2, uuid: "a", limit: "upper", status: "active" }],
		} as never;
		expect(alignSeries(series, 3)).toEqual({
			y: [1, null, 3],
			baseline: [1, 1, 1],
			alerts: [2],
			raised: [{ index: 2, uuid: "a", status: "active", limit: "upper" }],
		});
	});
});

describe("groupsOf", () => {
	// Kills a group header placed by the group's index rather than the lines before it.
	test("starts each group after the lines of the ones before it", () => {
		const batch = reportFixture([lineFixture()], {
			benchmarks: [
				{ uuid: "b", name: "blake3", slug: "blake3" },
				{ uuid: "s", name: "sha256", slug: "sha256" },
			],
			groups: [
				{ key: 1, lines: 4, variants: 2, alerts: 1 },
				{ key: 0, lines: 6, variants: 3, alerts: 0 },
			],
		});
		expect(groupsOf(batch, "benchmark")).toEqual([
			{ name: "sha256", lines: 4, variants: 2, alerts: 1, start: 0 },
			{ name: "blake3", lines: 6, variants: 3, alerts: 0, start: 4 },
		]);
	});

	// Kills naming a measure group from the benchmarks table.
	test("names a measure group from the measures table", () => {
		const batch = reportFixture([lineFixture()], {
			groups: [{ key: 1, lines: 2, variants: 2, alerts: 0 }],
		});
		expect(groupsOf(batch, "measure")[0]?.name).toBe("Throughput");
	});
});

describe("plotOf", () => {
	// Kills an expanded plot that loses the page's reports, the line's limits, or its alert.
	test("draws the line over its page's points with its limits and alerts", () => {
		const batch = reportFixture([
			lineFixture({
				alert: { uuid: "a", limit: "upper", status: "active" } as never,
				history: {
					y: [19.3, 19.5, 20.6],
					upper: [19.8, 19.8, 19.9],
					alerts: [{ index: 2, uuid: "a", limit: "upper", status: "active" }],
				} as never,
			}),
		]);
		const [line] = linesOf(batch);
		const data = plotOf(line as never, "main", "ubuntu-latest");
		expect(data.x).toEqual(batch.points.x);
		expect(data.report).toEqual([0, 1, 2]);
		expect(data.reports[2]).toMatchObject({ version: 24, hash: "9c1f2e4" });
		expect(data.measures).toEqual([
			{ name: "Latency", units: "nanoseconds (ns)" },
		]);
		expect(data.lines).toEqual([
			expect.objectContaining({
				benchmark: "blake3",
				branch: "main",
				testbed: "ubuntu-latest",
				alerting: true,
				y: [19.3, 19.5, 20.6],
				upper: [19.8, 19.8, 19.9],
				alerts: [2],
			}),
		]);
	});
});
