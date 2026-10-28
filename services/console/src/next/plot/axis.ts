import { extent } from "./columns";
import { type AxisTier, axisTier } from "./format";
import { yRange, yScale, yTicks } from "./scale";
import type { PlotLine, PlotScale } from "./types";

/** The most ticks a y axis draws. */
const TICKS = 5;

/** The tier of the largest tick an axis draws from `min` to `max`. */
export const rangeTier = (
	min: number,
	max: number,
	units: string,
	mode: PlotScale,
	count = TICKS,
): AxisTier => {
	const kind = yScale(mode, min, max);
	const [lo, hi] = yRange(kind, min, max);
	return axisTier(
		Math.max(0, ...yTicks(kind, lo, hi, count).ticks.map(Math.abs)),
		units,
	);
};

/** The ticks an axis draws for `lines`, in the tier of the largest. */
export const axisTicks = (
	lines: readonly Pick<PlotLine, "y" | "lower" | "upper">[],
	units: string,
	mode: PlotScale,
	count = TICKS,
) => {
	const range = extent(lines);
	if (!range) {
		return undefined;
	}
	const [min, max] = range;
	const tier = rangeTier(min, max, units, mode, count);
	const kind = yScale(mode, min, max);
	const [lo, hi] = yRange(kind, min, max);
	return {
		tier,
		...yTicks(kind, lo / tier.factor, hi / tier.factor, count),
	};
};
