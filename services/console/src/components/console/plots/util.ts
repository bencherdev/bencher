import { PerfQueryKey, PlotKey, type JsonPlot } from "../../../types/bencher";

// Preload plots that are within this many pixels of the viewport,
// so they begin fetching just before they scroll into view.
export const PLOT_PRELOAD_MARGIN = 200;

// Whether a plot's bounding box is within (or near) the vertical viewport.
// Used to eagerly load plots that are already on screen at mount time instead
// of waiting on the IntersectionObserver's initial callback, which is not
// reliably delivered for elements that are already in view and never
// subsequently move. That gap is why pinned plots could stay stuck on the
// loading skeleton until a manual refresh re-mounted them.
export const isNearViewport = (
	rect: { top: number; bottom: number },
	viewportHeight: number,
	margin = PLOT_PRELOAD_MARGIN,
): boolean => rect.top < viewportHeight + margin && rect.bottom > -margin;

export const plotQueryString = (plot: JsonPlot) => {
	const newParams = new URLSearchParams();
	newParams.set(PlotKey.LowerValue, plot?.lower_value.toString());
	newParams.set(PlotKey.UpperValue, plot?.upper_value.toString());
	newParams.set(PlotKey.LowerBoundary, plot?.lower_boundary.toString());
	newParams.set(PlotKey.UpperBoundary, plot?.upper_boundary.toString());
	newParams.set(PlotKey.XAxis, plot?.x_axis);
	newParams.set(PlotKey.YAxis, plot?.y_axis);
	newParams.set(PerfQueryKey.Branches, plot?.branches.toString());
	newParams.set(PerfQueryKey.Testbeds, plot?.testbeds.toString());
	newParams.set(PerfQueryKey.Benchmarks, plot?.benchmarks.toString());
	newParams.set(PerfQueryKey.Measures, plot?.measures.toString());
	const now = Date.now();
	newParams.set(
		PerfQueryKey.StartTime,
		(now - (plot?.window ?? 0) * 1_000).toString(),
	);
	newParams.set(PerfQueryKey.EndTime, now.toString());
	return newParams.toString();
};
