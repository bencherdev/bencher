import { DASHED, type Shape } from "./series";
import type { Palette } from "./theme";

// Shapes every canvas of the plot shares, sized in CSS pixels times `ratio`.

const BAND_ALPHA = 0.16;
const LIMIT_WIDTH = 1.3;
const MARK_STROKE = 1.5;

export interface Point {
	x: number;
	y: number;
}

/** A shaded run of a band: its top and bottom edge at each x. */
export interface Area {
	x: number;
	top: number;
	bottom: number;
}

export const fillAreas = (
	ctx: CanvasRenderingContext2D,
	areas: readonly (readonly Area[])[],
	color: string,
): void => {
	ctx.save();
	ctx.globalAlpha = BAND_ALPHA;
	ctx.fillStyle = color;
	for (const area of areas) {
		const [first] = area;
		if (!first || area.length < 2) {
			continue;
		}
		ctx.beginPath();
		ctx.moveTo(first.x, first.top);
		for (const { x, top } of area) {
			ctx.lineTo(x, top);
		}
		for (let index = area.length - 1; index >= 0; index--) {
			const point = area[index] as Area;
			ctx.lineTo(point.x, point.bottom);
		}
		ctx.closePath();
		ctx.fill();
	}
	ctx.restore();
};

export const strokeLimits = (
	ctx: CanvasRenderingContext2D,
	runs: readonly (readonly Point[])[],
	color: string,
	ratio: number,
): void => {
	ctx.save();
	ctx.strokeStyle = color;
	ctx.lineWidth = LIMIT_WIDTH * ratio;
	ctx.setLineDash(DASHED.map((length) => length * ratio));
	for (const run of runs) {
		const [first] = run;
		if (!first || run.length < 2) {
			continue;
		}
		ctx.beginPath();
		ctx.moveTo(first.x, first.y);
		for (const { x, y } of run) {
			ctx.lineTo(x, y);
		}
		ctx.stroke();
	}
	ctx.restore();
};

/** An alert marker: a filled circle ringed in the plot's ground, with an exclamation mark. */
export const drawAlert = (
	ctx: CanvasRenderingContext2D,
	{ x, y }: Point,
	radius: number,
	palette: Palette,
	muted: boolean,
): void => {
	ctx.save();
	ctx.beginPath();
	ctx.arc(x, y, radius, 0, 2 * Math.PI);
	ctx.fillStyle = muted ? palette.muted : palette.marker;
	ctx.fill();
	ctx.lineWidth = (MARK_STROKE * radius) / 7;
	ctx.strokeStyle = palette.plot;
	ctx.stroke();
	const width = radius * 0.28;
	ctx.fillStyle = palette.onMarker;
	ctx.fillRect(x - width / 2, y - radius * 0.56, width, radius * 0.7);
	ctx.fillRect(x - width / 2, y + radius * 0.32, width, radius * 0.26);
	ctx.restore();
};

/** A point's mark in its line's shape; a hollow one is filled with the plot's ground. */
export const drawMark = (
	ctx: CanvasRenderingContext2D,
	{ x, y }: Point,
	shape: Shape,
	radius: number,
	color: string,
	palette: Palette,
	ratio: number,
): void => {
	ctx.beginPath();
	switch (shape) {
		case "square":
			ctx.rect(x - radius, y - radius, 2 * radius, 2 * radius);
			break;
		case "triangle":
			ctx.moveTo(x, y - radius - ratio);
			ctx.lineTo(x + radius + 0.8 * ratio, y + radius);
			ctx.lineTo(x - radius - 0.8 * ratio, y + radius);
			ctx.closePath();
			break;
		default:
			ctx.arc(x, y, radius, 0, 2 * Math.PI);
	}
	if (shape === "hollow") {
		ctx.fillStyle = palette.plot;
		ctx.fill();
		ctx.lineWidth = MARK_STROKE * ratio;
		ctx.strokeStyle = color;
		ctx.stroke();
	} else {
		ctx.fillStyle = color;
		ctx.fill();
	}
};
