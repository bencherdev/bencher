import { eachReport, pickPoints, pointsOf, throughOf } from "./columns";
import type { PlotData, PlotLine } from "./types";

/** Each line with the values it runs through, where a report ran it more than once. */
export const withThrough = (data: PlotData): PlotData =>
	data.iteration
		? {
				...data,
				lines: data.lines.map((line): PlotLine => {
					const through = throughOf(
						line.y,
						line.alerts,
						data.report,
						data.iteration,
					);
					return through ? { ...line, through } : line;
				}),
			}
		: data;

/** A report's iterations as one bar: from its lowest value to its highest, at the x of `index`. */
interface RangeBar {
	index: number;
	low: number;
	high: number;
}

export const rangeBars = (
	y: readonly (number | null)[],
	report: readonly number[],
): RangeBar[] => {
	const bars: RangeBar[] = [];
	eachReport(report, (index, end) => {
		const values = y
			.slice(index, end)
			.filter((value): value is number => value != null);
		if (values.length > 1) {
			bars.push({
				index,
				low: Math.min(...values),
				high: Math.max(...values),
			});
		}
	});
	return bars;
};

/** Only iteration `n` of every report, counting from 1. */
export const onlyIteration = (data: PlotData, n: number): PlotData =>
	pickPoints(data, pointsOf(data.iteration, data.x.length, n));
