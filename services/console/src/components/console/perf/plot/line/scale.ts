import * as d3 from "d3";
import { YAxis } from "../../../../../types/bencher";

export type YScale =
	| d3.ScalePower<number, number, never>
	| d3.ScaleLogarithmic<number, number, never>;

// Build the y-axis scale for the selected mode.
// `min` and `max` are already divided by the unit factor.
export const get_y_scale = (
	y_axis: YAxis,
	min: number,
	max: number,
): YScale => {
	switch (y_axis) {
		case YAxis.Linear:
			// A true linear scale that shows magnitudes as-is.
			return d3.scalePow().exponent(1).domain([min, max]).nice();
		case YAxis.Log:
			// A genuine logarithmic scale. The logarithm is undefined for zero
			// or negative values, so fall back to the auto scale in that case
			// rather than breaking the plot.
			if (min > 0) {
				return d3.scaleLog().domain([min, max]).nice();
			}
			return auto_y_scale(min, max);
		default:
			// Auto: adapt the power-scale exponent to the data spread.
			return auto_y_scale(min, max);
	}
};

// Use pow scaling to allow users to more easily reason on graphs with highly
// differentiated values. If the min is less than 10 times smaller than the max,
// use a linear scale.
//
// See: https://observablehq.com/plot/features/scales#continuous-scales
const auto_y_scale = (min: number, max: number) => {
	const relativeDifference = max / min;
	const exponent =
		relativeDifference < 10
			? 1
			: Math.max(1 / Math.log10(relativeDifference), 1 / 3);
	return d3.scalePow().exponent(exponent).domain([min, max]).nice();
};
