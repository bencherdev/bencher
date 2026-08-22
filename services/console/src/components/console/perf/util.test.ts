import { describe, expect, test } from "vitest";
import {
	type JsonBoundary,
	type JsonReport,
	type JsonReportMeasure,
	PerfQueryKey,
	PlotKey,
} from "../../../types/bencher";
import {
	CLEAR_PARAM,
	DEFAULT_REPORT_HISTORY,
	PLOT_PARAM,
	REPORT_PARAM,
	firstReportParams,
} from "./util";

const REPORT_UUID = "a1a1a1a1-0000-4000-8000-000000000001";
const BRANCH_UUID = "b2b2b2b2-0000-4000-8000-000000000002";
const HEAD_UUID = "c3c3c3c3-0000-4000-8000-000000000003";
const TESTBED_UUID = "d4d4d4d4-0000-4000-8000-000000000004";
const SPEC_UUID = "e5e5e5e5-0000-4000-8000-000000000005";
const BENCHMARK_UUID = "f6f6f6f6-0000-4000-8000-000000000006";
const OTHER_BENCHMARK_UUID = "07070707-0000-4000-8000-000000000007";
const MEASURE_UUID = "18181818-0000-4000-8000-000000000008";
const OTHER_MEASURE_UUID = "29292929-0000-4000-8000-000000000009";

const START_TIME = "2026-08-20T12:00:00Z";
const END_TIME = "2026-08-20T12:05:00Z";
const START_TIME_MS = Date.parse(START_TIME);
const END_TIME_MS = Date.parse(END_TIME);

const reportMeasure = (
	uuid: string,
	boundary?: JsonBoundary,
): JsonReportMeasure =>
	({
		measure: { uuid, name: "Latency", slug: "latency", units: "nanoseconds" },
		metric: { uuid, value: 1.0 },
		boundary,
	}) as unknown as JsonReportMeasure;

const report = (measures: JsonReportMeasure[]): JsonReport =>
	({
		uuid: REPORT_UUID,
		branch: { uuid: BRANCH_UUID, head: { uuid: HEAD_UUID } },
		testbed: { uuid: TESTBED_UUID, spec: { uuid: SPEC_UUID } },
		start_time: START_TIME,
		end_time: END_TIME,
		results: [
			[
				{ iteration: 0, benchmark: { uuid: BENCHMARK_UUID }, measures },
				{
					iteration: 0,
					benchmark: { uuid: OTHER_BENCHMARK_UUID },
					measures: [reportMeasure(OTHER_MEASURE_UUID)],
				},
			],
		],
	}) as unknown as JsonReport;

describe("firstReportParams", () => {
	test("selects the first measure of the first result of the first iteration", () => {
		const params = firstReportParams(report([reportMeasure(MEASURE_UUID)]));
		expect(params[PerfQueryKey.Measures]).toBe(MEASURE_UUID);
	});

	test("plots every benchmark in the first iteration", () => {
		const params = firstReportParams(report([reportMeasure(MEASURE_UUID)]));
		expect(params[PerfQueryKey.Benchmarks]).toBe(
			`${BENCHMARK_UUID},${OTHER_BENCHMARK_UUID}`,
		);
	});

	test("a lower limit turns on the lower boundary only", () => {
		const params = firstReportParams(
			report([reportMeasure(MEASURE_UUID, { lower_limit: 1.0 })]),
		);
		expect(params[PlotKey.LowerBoundary]).toBe(true);
		expect(params[PlotKey.UpperBoundary]).toBe(false);
	});

	test("an upper limit turns on the upper boundary only", () => {
		const params = firstReportParams(
			report([reportMeasure(MEASURE_UUID, { upper_limit: 9.0 })]),
		);
		expect(params[PlotKey.LowerBoundary]).toBe(false);
		expect(params[PlotKey.UpperBoundary]).toBe(true);
	});

	test("both limits turn on both boundaries", () => {
		const params = firstReportParams(
			report([
				reportMeasure(MEASURE_UUID, { lower_limit: 1.0, upper_limit: 9.0 }),
			]),
		);
		expect(params[PlotKey.LowerBoundary]).toBe(true);
		expect(params[PlotKey.UpperBoundary]).toBe(true);
	});

	test("a boundary with only a baseline turns on neither boundary", () => {
		const params = firstReportParams(
			report([reportMeasure(MEASURE_UUID, { baseline: 5.0 })]),
		);
		expect(params[PlotKey.LowerBoundary]).toBe(false);
		expect(params[PlotKey.UpperBoundary]).toBe(false);
	});

	test("no boundary at all turns on neither boundary", () => {
		const params = firstReportParams(report([reportMeasure(MEASURE_UUID)]));
		expect(params[PlotKey.LowerBoundary]).toBe(false);
		expect(params[PlotKey.UpperBoundary]).toBe(false);
	});

	test("only the first measure decides the boundaries", () => {
		const params = firstReportParams(
			report([
				reportMeasure(MEASURE_UUID),
				reportMeasure(OTHER_MEASURE_UUID, { upper_limit: 9.0 }),
			]),
		);
		expect(params[PerfQueryKey.Measures]).toBe(MEASURE_UUID);
		expect(params[PlotKey.UpperBoundary]).toBe(false);
	});

	test("the plot window ends at the report and reaches back the default history", () => {
		const params = firstReportParams(report([reportMeasure(MEASURE_UUID)]));
		expect(params[REPORT_PARAM]).toBe(REPORT_UUID);
		expect(params[PerfQueryKey.Branches]).toBe(BRANCH_UUID);
		expect(params[PerfQueryKey.Heads]).toBe(HEAD_UUID);
		expect(params[PerfQueryKey.Testbeds]).toBe(TESTBED_UUID);
		expect(params[PerfQueryKey.Specs]).toBe(SPEC_UUID);
		expect(params[PerfQueryKey.StartTime]).toBe(
			START_TIME_MS - DEFAULT_REPORT_HISTORY,
		);
		expect(params[PerfQueryKey.EndTime]).toBe(END_TIME_MS);
		expect(params[PLOT_PARAM]).toBe(null);
		expect(params[PlotKey.LowerValue]).toBe(null);
		expect(params[PlotKey.UpperValue]).toBe(null);
		expect(params[CLEAR_PARAM]).toBe(true);
	});
});
