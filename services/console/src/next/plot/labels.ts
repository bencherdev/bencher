import { isServer } from "solid-js/web";

// Axis labels are measured as the canvas draws them, so an axis is never narrower than its widest label.

const FONT_PX = 11;
/** The axis gap uPlot leaves between labels and the plot, plus a margin at the canvas edge. */
const PAD = 12;
const MIN_WIDTH = 30;

let measurer: CanvasRenderingContext2D | null | undefined;

/** uPlot scales the font by the pixel ratio itself, and only a whole pixel size. */
export const axisFont = (family: string): string => `${FONT_PX}px ${family}`;

/** The width in CSS pixels a y axis needs to draw `labels` in `font`. */
export const axisWidth = (
	labels: readonly (string | null)[],
	font: string,
): number => {
	if (isServer) {
		return MIN_WIDTH;
	}
	let widest = 0;
	for (const label of labels) {
		if (label) {
			widest = Math.max(widest, labelWidth(label, font));
		}
	}
	return Math.max(MIN_WIDTH, Math.ceil(widest) + PAD);
};

/** The width of `label` drawn in `font`, in the pixels the font's size is given in; in a browser only. */
export const labelWidth = (label: string, font: string): number => {
	measurer ??= document.createElement("canvas").getContext("2d");
	if (!measurer) {
		return 0;
	}
	measurer.font = font;
	return measurer.measureText(label).width;
};
