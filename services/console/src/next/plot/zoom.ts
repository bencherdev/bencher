import type { PlotLine } from "./types";

/** An x range a plot is zoomed to, its smaller end first. */
export type XRange = readonly [number, number];

/** The span `[start, end)` of the indices of `x`, which ascends, inside `range`; every index when there is none. */
export const within = (
	x: readonly number[],
	range: XRange | null,
): [number, number] =>
	range
		? [firstPast(x, range[0], false), firstPast(x, range[1], true)]
		: [0, x.length];

/** The first index whose x is at least `value`, or past it when `strictly`. */
const firstPast = (
	x: readonly number[],
	value: number,
	strictly: boolean,
): number => {
	let low = 0;
	let high = x.length;
	while (low < high) {
		const middle = (low + high) >>> 1;
		const at = x[middle] as number;
		if (strictly ? at <= value : at < value) {
			low = middle + 1;
		} else {
			high = middle;
		}
	}
	return low;
};

/** A line's points at the indices `[start, end)`, with its lower and upper bounds at the same indices. */
export const pointsIn = (
	{ y, lower, upper }: Pick<PlotLine, "y" | "lower" | "upper">,
	start: number,
	end: number,
): Pick<PlotLine, "y" | "lower" | "upper"> => ({
	y: y.slice(start, end),
	...(lower && { lower: lower.slice(start, end) }),
	...(upper && { upper: upper.slice(start, end) }),
});

/** Whether any of the `columns` has points at 2 or more x inside `range`. */
export const holdsTwo = (
	x: readonly number[],
	columns: readonly (readonly (number | null)[])[],
	range: XRange,
): boolean => {
	const [start, end] = within(x, range);
	for (const column of columns) {
		let first: number | undefined;
		for (let index = start; index < end; index++) {
			if (column[index] == null) {
				continue;
			}
			if (first === undefined) {
				first = x[index];
			} else if (x[index] !== first) {
				return true;
			}
		}
	}
	return false;
};

/** The fraction of a range's width under which a narrower range is the same range: what the narrowest pinch's search leaves over. */
const SLIVER = 1e-6;

/** Whether `range` is narrower than `than` by more than a sliver. */
export const narrower = (range: XRange, than: XRange): boolean =>
	range[1] - range[0] < (than[1] - than[0]) * (1 - SLIVER);

/** `range` scaled by `ratio` around the value at `at`, its fraction across the range, which keeps its place. */
export const scaled = (
	[min, max]: XRange,
	at: number,
	ratio: number,
): XRange => {
	const width = max - min;
	const anchor = min + at * width;
	return [anchor - at * width * ratio, anchor + (1 - at) * width * ratio];
};

/**
 * The range a pinch shows: `start` scaled by `ratio` around `at`, never past
 * `full`, and on a pinch in never narrower than the narrowest that holds 2
 * points of a column, or `start` itself when that is no more than a sliver wider.
 * None when `start` holds fewer.
 */
export const pinched = (
	x: readonly number[],
	columns: readonly (readonly (number | null)[])[],
	start: XRange,
	at: number,
	ratio: number,
	full: XRange,
): XRange | null => {
	let range = scaled(start, at, ratio);
	if (ratio < 1 && !holdsTwo(x, columns, range)) {
		if (!holdsTwo(x, columns, start)) {
			return null;
		}
		// The range narrows as the ratio does, so halving the gap finds where it stops holding 2.
		let low = ratio;
		let high = 1;
		for (let step = 0; step < 32; step++) {
			const middle = (low + high) / 2;
			if (holdsTwo(x, columns, scaled(start, at, middle))) {
				high = middle;
			} else {
				low = middle;
			}
		}
		range = scaled(start, at, high);
		// Already at its narrowest, the search still finds a sliver narrower, which is no step at all.
		if (!narrower(range, start)) {
			range = start;
		}
	}
	return [Math.max(range[0], full[0]), Math.min(range[1], full[1])];
};
