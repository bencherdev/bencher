import { type JsonReport, PerfQueryKey, PlotKey } from "../../../types/bencher";
import { arrayToString, dateTimeMillis } from "../../../util/convert";
import type { SetParams } from "../../../util/url";

// Console UI state query params
export const REPORT_PARAM = "report";
export const PLOT_PARAM = "plot";
export const CLEAR_PARAM = "clear";

// The most benchmarks to plot by default
const DEFAULT_REPORT_BENCHMARKS = 10;
// 4 weeks
export const DEFAULT_REPORT_HISTORY = 28 * 24 * 60 * 60 * 1000;

// The query params for the default perf plot, bootstrapped from the first report.
export const firstReportParams = (first_report: JsonReport): SetParams => {
	const first = 0;
	const benchmarks = first_report?.results?.[first]
		?.map((iteration) => iteration?.benchmark?.uuid)
		.slice(0, DEFAULT_REPORT_BENCHMARKS);
	// The boundary limits live on the report measure, not on the measure itself.
	const first_report_measure =
		first_report?.results?.[first]?.[first]?.measures?.[first];
	const start_time = dateTimeMillis(first_report?.start_time);
	return {
		[REPORT_PARAM]: first_report?.uuid,
		[PerfQueryKey.Branches]: first_report?.branch?.uuid,
		[PerfQueryKey.Heads]: first_report?.branch?.head?.uuid,
		[PerfQueryKey.Testbeds]: first_report?.testbed?.uuid,
		[PerfQueryKey.Specs]: first_report?.testbed?.spec?.uuid,
		[PerfQueryKey.Benchmarks]: arrayToString(benchmarks ?? []),
		[PerfQueryKey.Measures]: first_report_measure?.measure?.uuid,
		[PLOT_PARAM]: null,
		[PerfQueryKey.StartTime]: start_time
			? start_time - DEFAULT_REPORT_HISTORY
			: null,
		[PerfQueryKey.EndTime]: dateTimeMillis(first_report?.end_time),
		[PlotKey.LowerValue]: null,
		[PlotKey.UpperValue]: null,
		[PlotKey.LowerBoundary]:
			typeof first_report_measure?.boundary?.lower_limit === "number",
		[PlotKey.UpperBoundary]:
			typeof first_report_measure?.boundary?.upper_limit === "number",
		[CLEAR_PARAM]: true,
	};
};
