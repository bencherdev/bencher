import { describe, expect, test } from "vitest";
import { lineKey } from "../query/line";
import { alignSeries, groupsOf, linesOf, plotOf } from "./lines";
import { lineFixture, reportFixture } from "./testing";

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

describe("alignSeries", () => {
	// Kills drawing a thinned history's values at their own positions instead of the page's points.
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
