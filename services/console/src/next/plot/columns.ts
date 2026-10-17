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
	return { ...pickPoints(data, order), x: order.map(versionOf) };
};

/** The data on only the points `order` names, in its order. */
export const pickPoints = (
	data: PlotData,
	order: readonly number[],
): PlotData => {
	const take = <T>(column: readonly T[]) =>
		order.map((from) => column[from] as T);
	return {
		...data,
		x: take(data.x),
		report: take(data.report),
		...(data.iteration && { iteration: take(data.iteration) }),
		lines: data.lines.map((line) => pickLine(line, order)),
	};
};

/** A line's columns on only the points `order` names, each alert moved with its point. */
export const pickLine = <
	L extends Pick<
		PlotLine,
		"y" | "baseline" | "lower" | "upper" | "alerts" | "raised"
	>,
>(
	line: L,
	order: readonly number[],
): L => {
	const position: (number | undefined)[] = [];
	order.forEach((from, to) => {
		position[from] = to;
	});
	const take = (column: Column) => order.map((from) => column[from] ?? null);
	return {
		...line,
		y: take(line.y),
		...(line.baseline && { baseline: take(line.baseline) }),
		...(line.lower && { lower: take(line.lower) }),
		...(line.upper && { upper: take(line.upper) }),
		alerts: line.alerts
			.flatMap((index) => position[index] ?? [])
			.sort((a, b) => a - b),
		...(line.raised && {
			raised: line.raised.flatMap((alert) => {
				const index = position[alert.index];
				return index === undefined ? [] : [{ ...alert, index }];
			}),
		}),
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

/** Calls `each` with the span `[start, end)` of each report's points, which run together. */
export const eachReport = (
	report: readonly number[],
	each: (start: number, end: number) => void,
) => {
	let start = 0;
	for (let end = 1; end <= report.length; end++) {
		if (end === report.length || report[end] !== report[start]) {
			each(start, end);
			start = end;
		}
	}
};

/**
 * The values a line runs through, one per report: the iteration that alerted,
 * else the last, as a report's points run in iteration order. Undefined when
 * no report ran the line twice, so it draws as `y`.
 */
export const throughOf = (
	y: readonly (number | null)[],
	alerts: readonly number[],
	report: readonly number[],
	iteration: readonly number[] | undefined,
): (number | null)[] | undefined => {
	if (!iteration) {
		return undefined;
	}
	const through = y.slice();
	let several = false;
	eachReport(report, (start, end) => {
		let kept: number | undefined;
		for (let point = start; point < end; point++) {
			if (y[point] == null) {
				continue;
			}
			if (kept !== undefined) {
				several = true;
				if (alerts.includes(kept) && !alerts.includes(point)) {
					through[point] = null;
					continue;
				}
				through[kept] = null;
			}
			kept = point;
		}
	});
	return several ? through : undefined;
};

/** The points of iteration `n`, counting from 1. */
export const pointsOf = (
	iteration: readonly number[] | undefined,
	length: number,
	n: number,
) =>
	Array.from({ length }, (_, index) => index).filter(
		(index) => (iteration?.[index] ?? 0) === n - 1,
	);
