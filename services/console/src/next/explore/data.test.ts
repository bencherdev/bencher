import { describe, expect, test } from "vitest";
import {
	AlertStatus,
	BoundaryLimit,
	type JsonConsolePerf,
	ModelTest,
} from "../../types/bencher";
import { lineKey } from "../query/line";
import { metricNames, plotData } from "./data";

const uuid = (n: number) =>
	`00000000-0000-4000-8000-${n.toString(16).padStart(12, "0")}`;

const alert = (index: number, status: AlertStatus) => ({
	index,
	uuid: uuid(100 + index),
	limit: BoundaryLimit.Upper,
	status,
});

const PERF: JsonConsolePerf = {
	window: { start_time: 0, end_time: 3, clamped: false },
	total: 3,
	points: { x: [1000, 2000, 3000], report: [0, 1, 1] },
	reports: [
		{ uuid: uuid(20), version: 7, hash: "abc" },
		{ uuid: uuid(21), version: 8 },
	],
	branches: [{ uuid: uuid(1), name: "main", slug: "main", head: uuid(11) }],
	testbeds: [{ uuid: uuid(3), name: "linux", slug: "linux" }],
	benchmarks: [{ uuid: uuid(5), name: "blake3", slug: "blake3" }],
	variants: [
		{ uuid: uuid(30), benchmark: 0, parameters: { simd: "avx2", size: 64 } },
		{ uuid: uuid(31), benchmark: 0, parameters: { simd: "sse", size: 64 } },
	],
	measures: [
		{ uuid: uuid(7), name: "Latency", slug: "latency", units: "ns" },
		{ uuid: uuid(8), name: "Throughput", slug: "throughput", units: "B/s" },
	],
	models: [
		{
			uuid: uuid(40),
			threshold: uuid(41),
			test: ModelTest.TTest,
			upper_boundary: 0.99,
		},
		{
			uuid: uuid(42),
			threshold: uuid(43),
			test: ModelTest.Percentage,
			lower_boundary: 0.1,
			upper_boundary: 0.1,
		},
	],
	lines: [
		{
			branch: 0,
			testbed: 0,
			benchmark: 0,
			variant: 0,
			measure: 0,
			metric: "value",
			model: 0,
			series: {
				y: [1, null, 3],
				baseline: [null, 1, 1],
				upper: [null, 2, 2],
				alerts: [alert(2, AlertStatus.Active)],
			},
		},
		{
			branch: 0,
			testbed: 0,
			benchmark: 0,
			variant: 1,
			measure: 1,
			metric: "p99",
			model: 1,
			series: {
				y: [4, 5, 6],
				lower: [1, 1, 1],
				alerts: [alert(0, AlertStatus.Dismissed)],
			},
		},
		{
			branch: 0,
			testbed: 0,
			benchmark: 0,
			variant: 1,
			measure: 0,
			metric: "value",
			series: { y: [null, null, null], alerts: [] },
		},
	],
};

describe("plotData", () => {
	// Kills a column, a report, or an axis read from the wrong place.
	test("keep the points, the reports, and each line's columns", () => {
		const data = plotData(PERF);
		expect(data.x).toEqual([1000, 2000, 3000]);
		expect(data.report).toEqual([0, 1, 1]);
		expect(data.reports).toEqual(PERF.reports);
		expect(data.measures).toEqual([
			{ name: "Latency", units: "ns" },
			{ name: "Throughput", units: "B/s" },
		]);
		const [first, second] = data.lines;
		expect(first).toMatchObject({
			benchmark: "blake3",
			parameters: { simd: "avx2", size: 64 },
			measure: 0,
			metric: "value",
			branch: "main",
			testbed: "linux",
			y: [1, null, 3],
			baseline: [null, 1, 1],
			upper: [null, 2, 2],
			alerts: [2],
		});
		expect(first?.lower).toBeUndefined();
		expect(second).toMatchObject({ measure: 1, lower: [1, 1, 1], alerts: [0] });
	});

	// Kills a line id from names or positions, which would not match the key in a link.
	test("name each line by its key", () => {
		const [first, second] = plotData(PERF).lines;
		expect(first?.id).toBe(
			lineKey({
				branch: uuid(1),
				testbed: uuid(3),
				benchmark: uuid(5),
				parameters: { simd: "avx2", size: 64 },
				measure: uuid(7),
				metric: "value",
			}),
		);
		expect(second?.id).toBe(
			lineKey({
				branch: uuid(1),
				testbed: uuid(3),
				benchmark: uuid(5),
				parameters: { simd: "sse", size: 64 },
				measure: uuid(8),
				metric: "p99",
			}),
		);
	});

	// Kills counting a dismissed alert as alerting now, or missing an active one.
	test("call a line alerting only while one of its alerts is active", () => {
		expect(plotData(PERF).lines.map(({ alerting }) => alerting)).toEqual([
			true,
			false,
			false,
		]);
	});

	// Kills naming the wrong test or side, or a model for a line no threshold checked.
	test("name the threshold model behind each line", () => {
		expect(plotData(PERF).lines.map(({ model }) => model)).toEqual([
			"t-test upper",
			"percentage lower and upper",
			undefined,
		]);
	});
});

describe("metricNames", () => {
	// Kills repeating a name or reordering what the response drew.
	test("list each drawn metric name once, in drawing order", () => {
		expect(metricNames(PERF)).toEqual(["value", "p99"]);
	});
});
