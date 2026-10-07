import type { PlotData, PlotLine, PlotXAxis } from "./types";

type Column = readonly (number | null)[];

// Version numbers are not in time order across branches and uPlot takes x ascending, so a version axis sorts every column.
export const alignToAxis = (data: PlotData, axis: PlotXAxis): PlotData => {
	if (axis === "date") {
		return data;
	}
	const versionOf = (index: number) =>
		data.reports[data.report[index] ?? -1]?.version ?? 0;
	const order = data.x
		.map((_, index) => index)
		.sort((a, b) => versionOf(a) - versionOf(b) || a - b);
	const position = new Array<number>(order.length);
	order.forEach((from, to) => {
		position[from] = to;
	});
	const take = (column: Column): (number | null)[] =>
		order.map((from) => column[from] ?? null);
	return {
		...data,
		x: order.map(versionOf),
		report: order.map((from) => data.report[from] ?? 0),
		lines: data.lines.map(
			(line): PlotLine => ({
				...line,
				y: take(line.y),
				...(line.baseline && { baseline: take(line.baseline) }),
				...(line.lower && { lower: take(line.lower) }),
				...(line.upper && { upper: take(line.upper) }),
				alerts: line.alerts
					.map((index) => position[index] ?? index)
					.sort((a, b) => a - b),
			}),
		),
	};
};

/** The span `[start, end)` of indices that share the x at `index`. */
export const sameX = (
	x: readonly number[],
	index: number,
): [number, number] => {
	const value = x[index];
	let start = index;
	while (start > 0 && x[start - 1] === value) {
		start--;
	}
	let end = index + 1;
	while (end < x.length && x[end] === value) {
		end++;
	}
	return [start, end];
};

/** The smallest and largest of the lines' values and limits, so a range always fits the band too. */
export const extent = (
	lines: readonly Pick<PlotLine, "y" | "lower" | "upper">[],
): [number, number] | undefined => {
	let min = Number.POSITIVE_INFINITY;
	let max = Number.NEGATIVE_INFINITY;
	for (const { y, lower, upper } of lines) {
		for (const column of [y, lower, upper]) {
			for (const value of column ?? []) {
				if (value != null) {
					min = Math.min(min, value);
					max = Math.max(max, value);
				}
			}
		}
	}
	return min <= max ? [min, max] : undefined;
};
