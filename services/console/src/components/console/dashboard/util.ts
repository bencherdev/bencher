import { PerfQueryKey, PlotKey, type JsonPlot } from "../../../types/bencher";

export const plotQueryString = (plot: JsonPlot) => {
	const newParams = new URLSearchParams();
	newParams.set(PlotKey.LowerValue, plot?.lower_value.toString());
	newParams.set(PlotKey.UpperValue, plot?.upper_value.toString());
	newParams.set(PlotKey.LowerBoundary, plot?.lower_boundary.toString());
	newParams.set(PlotKey.UpperBoundary, plot?.upper_boundary.toString());
	newParams.set(PlotKey.XAxis, plot?.x_axis);
	newParams.set(PerfQueryKey.Branches, plot?.branches.toString());
	newParams.set(PerfQueryKey.Testbeds, plot?.testbeds.toString());
	newParams.set(PerfQueryKey.Benchmarks, plot?.benchmarks.toString());
	newParams.set(PerfQueryKey.Measures, plot?.measures.toString());
	const now = new Date().getTime();
	newParams.set(
		PerfQueryKey.StartTime,
		(now - (plot?.window ?? 0) * 1_000).toString(),
	);
	newParams.set(PerfQueryKey.EndTime, now.toString());
	return newParams.toString();
};
