import { parameterKey } from "./key";
import type { PlotLayout, PlotLine } from "./types";

export type Layout = "single" | "dual" | "stacked";

export type Shape = "circle" | "hollow" | "square" | "triangle";

const SHAPES: readonly Shape[] = ["circle", "hollow", "square", "triangle"];

export const SLOTS = 8;

export interface LineStyle {
	/** The series color, `--color-data-categorical-1` to `-8`. */
	slot: number;
	/** Past eight series, a repeated color changes its point shape. */
	shape: Shape;
	/** The plot of a stacked layout that draws the line. */
	panel: number;
	/** 1 for the right axis of a dual layout. */
	axis: number;
}

/** Dual axis at two measures unless stacked is asked for, and stacked at three or more. */
export const layoutOf = (measures: number, requested: PlotLayout): Layout => {
	if (measures <= 1) {
		return "single";
	}
	return measures === 2 && requested === "dual" ? "dual" : "stacked";
};

/** The measures the lines draw, in axis order. */
export const measuresOf = (lines: readonly PlotLine[]): number[] =>
	[...new Set(lines.map(({ measure }) => measure))].sort((a, b) => a - b);

export const lineStyles = (
	lines: readonly PlotLine[],
	layout: Layout,
): LineStyle[] => {
	const measures = measuresOf(lines);
	// Every measure of one series shares its color, so a variant reads as one thing in both places.
	const series = new Map<string, number>();
	return lines.map((line) => {
		const key = seriesKey(line);
		let index = series.get(key);
		if (index === undefined) {
			index = series.size;
			series.set(key, index);
		}
		const position = measures.indexOf(line.measure);
		return {
			slot: (index % SLOTS) + 1,
			shape: SHAPES[Math.floor(index / SLOTS) % SHAPES.length] as Shape,
			panel: layout === "stacked" ? position : 0,
			axis: layout === "dual" ? position : 0,
		};
	});
};

const seriesKey = (line: PlotLine): string =>
	JSON.stringify([
		line.branch,
		line.testbed,
		line.benchmark,
		parameterKey(line.parameters),
		line.metric,
	]);

/** Dotted strokes mean the second axis and nothing else. */
export const DOTTED: readonly number[] = [0.1, 3.6];

/** Dashed strokes mean a boundary limit and nothing else. */
export const DASHED: readonly number[] = [5, 4];

export const lineDash = (style: LineStyle): readonly number[] | undefined =>
	style.axis === 1 ? DOTTED : undefined;
