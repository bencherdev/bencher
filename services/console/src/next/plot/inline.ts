import { bandGeometry, EDGE } from "./band";
import { extent } from "./columns";
import {
	type Area,
	drawAlert,
	fillAreas,
	type Point,
	strokeLimits,
} from "./draw";
import { transform, yRange, yScale } from "./scale";
import type { Palette } from "./theme";
import type { PlotLine } from "./types";

export type InlineLine = Pick<PlotLine, "y" | "lower" | "upper" | "alerts">;

export const INLINE_PAD = { left: 3, right: 8, top: 6, bottom: 6 };

const LINE_WIDTH = 1.5;
const ALERT_RADIUS = 4.6;

export interface InlineGeometry {
	points: (Point | null)[];
	band: { areas: Area[][]; lower: Point[][]; upper: Point[][] };
	alerts: Point[];
}

/** Where a row's history draws inside `width` by `height` CSS pixels, with the plot's own scale rule. */
export const inlineGeometry = (
	x: readonly number[],
	line: InlineLine,
	width: number,
	height: number,
): InlineGeometry => {
	const [min, max] = extent([line]) ?? [0, 1];
	const left = INLINE_PAD.left;
	const right = width - INLINE_PAD.right;
	const top = INLINE_PAD.top;
	const bottom = height - INLINE_PAD.bottom;
	const scale = yScale("auto", min, max);
	const { fwd } = transform(scale);
	const [lo, hi] = yRange(scale, min, max, 0.14, 0.16);
	const low = fwd(lo);
	const span = fwd(hi) - low;
	const first = x[0] ?? 0;
	const duration = (x[x.length - 1] ?? 0) - first;
	const toX = (index: number) =>
		duration > 0
			? left + (((x[index] ?? first) - first) / duration) * (right - left)
			: (left + right) / 2;
	const toY = (value: number) =>
		bottom - ((fwd(value) - low) / span) * (bottom - top);

	const points = line.y.map((value, index) =>
		value == null ? null : { x: toX(index), y: toY(value) },
	);
	const geometry = bandGeometry(line.y, line.lower, line.upper);
	const run = (indices: number[], column: PlotLine["lower"]) =>
		indices.map((index) => ({
			x: toX(index),
			y: toY(column?.[index] ?? 0),
		}));
	return {
		points,
		band: {
			areas: geometry.areas.map((area) =>
				area.map(({ index, top: high, bottom: low }) => ({
					x: toX(index),
					top: high === EDGE ? top : toY(high),
					bottom: low === EDGE ? bottom : toY(low),
				})),
			),
			lower: geometry.lower.map((indices) => run(indices, line.lower)),
			upper: geometry.upper.map((indices) => run(indices, line.upper)),
		},
		alerts: line.alerts.flatMap((index) => {
			const point = points[index];
			return point ? [point] : [];
		}),
	};
};

/** Draws a row's history: its band, its line, and its alert markers, muted for a dismissed row. */
export const drawInline = (
	ctx: CanvasRenderingContext2D,
	geometry: InlineGeometry,
	palette: Palette,
	muted: boolean,
): void => {
	const color = muted ? palette.muted : (palette.series[0] as string);
	fillAreas(ctx, geometry.band.areas, color);
	strokeLimits(ctx, [...geometry.band.lower, ...geometry.band.upper], color, 1);
	ctx.beginPath();
	let drawing = false;
	for (const point of geometry.points) {
		if (!point) {
			continue;
		}
		if (drawing) {
			ctx.lineTo(point.x, point.y);
		} else {
			ctx.moveTo(point.x, point.y);
			drawing = true;
		}
	}
	ctx.lineWidth = LINE_WIDTH;
	ctx.lineJoin = "round";
	ctx.lineCap = "round";
	ctx.strokeStyle = color;
	ctx.stroke();
	for (const point of geometry.alerts) {
		drawAlert(ctx, point, ALERT_RADIUS, palette, muted);
	}
};
