import { formatDate, formatWhen, formatWhenSeconds } from "./format";
import type { PlotXAxis } from "./types";

/** An x range a plot is zoomed to, its smaller end first. */
export type XRange = readonly [number, number];

const MINUTE = 60_000;
const DAY = 86_400_000;

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

/** The span a zoom shows: its dates, as the date axis labels them at that span, or the versions inside it. */
export const zoomSpan = ([min, max]: XRange, axis: PlotXAxis): string => {
	if (axis !== "date") {
		return `${Math.ceil(min)} to ${Math.floor(max)}`;
	}
	const span = max - min;
	const format =
		span < MINUTE ? formatWhenSeconds : span < DAY ? formatWhen : formatDate;
	return `${format(min)} to ${format(max)}`;
};
