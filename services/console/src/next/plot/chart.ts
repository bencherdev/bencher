import uPlot from "uplot";
import { bandGeometry, EDGE } from "./band";
import { extent, sameX } from "./columns";
import {
	drawAlert,
	drawMark,
	fillAreas,
	strokeLimits,
	strokeRanges,
} from "./draw";
import {
	type AxisTier,
	formatDate,
	formatTicks,
	formatWhen,
	formatWhenSeconds,
} from "./format";
import { rangeBars } from "./iterations";
import { axisFont, axisWidth, labelWidth } from "./labels";
import { transform, type YScale, yRange, yScale, yTicks } from "./scale";
import { type LineStyle, lineDash } from "./series";
import type { Palette } from "./theme";
import type {
	PlotAlert,
	PlotData,
	PlotLine,
	PlotScale,
	PlotSize,
	PlotXAxis,
} from "./types";
import { type XRange, within } from "./zoom";

// One uPlot per plot of a layout; bands, markers, focus, and the dual axis are drawn in its hooks.

const WIDTH = { full: 1.8, row: 1.8, tile: 1.5 } as const;
const FOCUSED_WIDTH = 2.4;
const DOTTED_WIDTH = 2.1;
// A canvas strokes a one pixel line far faster than a wider one, which a crowded plot redraws on every hover.
const HAIRLINE = 1;
const HAIRLINE_DOTS: readonly number[] = [1, 3];
/** Past this many visible lines a plot draws them as hairlines. */
const CROWDED = 16;
const ALERT_RADIUS = { full: 7, row: 7, tile: 5.5 } as const;
const NARROW_ALERT_RADIUS = 6.5;
const MARK_RADIUS = { full: 2.5, row: 2.5, tile: 2.1 } as const;
const NARROW_MARK_RADIUS = 2.3;
/** Points draw their marks only this many CSS pixels apart or more. */
const MARK_SPACING = 10;
/** How near, in CSS pixels, the pointer must come to a line to focus it. */
const HOVER_REACH = 16;
/** How far, in CSS pixels, a mouse or pen press moves before it is a drag rather than a click. */
const DRAG_REACH = 5;
const X_AXIS_HEIGHT = 24;
const TICK_SPACING = 50;
const X_TICK_SPACING = 120;
const MINUTE = 60_000;
const DAY = 86_400_000;

// Steps for uPlot to fit a y axis to, past its own, which stop near 1e32 and leave a larger range without labels.
const Y_INCRS = Array.from({ length: 632 }, (_, index) => 10 ** (index - 323))
	.flatMap((power) => [power, 2 * power, 5 * power])
	.filter(Number.isFinite);

export interface PanelAxis {
	measure: number;
	/** Line indices on this axis. */
	lines: readonly number[];
}

export interface PanelOptions {
	data: PlotData;
	/** Line indices this plot draws, in data order. */
	lines: readonly number[];
	styles: readonly LineStyle[];
	/** The left axis, then the right one of a dual layout. */
	axes: readonly PanelAxis[];
	scale: PlotScale;
	xAxis: PlotXAxis;
	showX: boolean;
	size: PlotSize;
	narrow: boolean;
	/** The width a y axis takes given what its drawn labels need; stacked plots share one so their plot areas line up. */
	axisWidth: (axis: number, need: number) => number;
	/** The id of the text that names the plot area's keys. */
	description: string;
	/** Shared by the plots of a stacked layout so their cursors move together. */
	syncKey: string | null;
	/** The measures this plot draws, as its name says them. */
	measures: string;
	width: number;
	height: number;
}

export interface PanelState {
	hidden: () => ReadonlySet<number>;
	focus: () => number | null;
	palette: () => Palette;
	pinned: () => boolean;
	index: () => number | null;
	/** An alert's status now, which mutes its marker once it is no longer active. */
	status: (alert: PlotAlert) => PlotAlert["status"];
	/** The tier a measure's axis prints its ticks in, from the lines it shows. */
	tier: (measure: number) => AxisTier;
	/** The x range the plot is zoomed to, or null for every point. */
	view: () => XRange | null;
}

/** Where the pointer or the keys put the cursor: a data index, which on a line is its point nearest the pointer, the nearest line, and the x in CSS pixels inside the plot area. */
export interface CursorEvent {
	index: number;
	line: number | null;
	x: number;
}

export interface PanelEvents {
	hover: (event: CursorEvent | null) => void;
	/** The keys put the cursor at `event` and pin it there. */
	pin: (event: CursorEvent) => void;
	/** A click, or a tap when `touch`, at `event`. */
	click: (event: CursorEvent, touch: boolean) => void;
	/** Enter or Space at the cursor's point. */
	toggle: () => void;
	close: () => void;
	/** A drag across the x range `range`, in data values. */
	zoom: (range: XRange) => void;
	/** A double click, which goes back to every point. */
	unzoom: () => void;
}

export interface Panel {
	/** The plot area inside the panel, in CSS pixels. */
	frame: () => { left: number; top: number; width: number };
	/** Draws again with the state's hidden lines, focus, and palette, synchronously. */
	update: () => void;
	/** Pins the cursor at a data index, on `line`'s point there when given, and on no line as the keys do. */
	pinAt: (index: number, line?: number | null) => void;
	clearCursor: () => void;
	setSize: (width: number, height: number) => void;
	/** The y of a line's value at an index, in CSS pixels inside the plot area. */
	yOf: (line: number, index: number) => number | null;
	/** Lays the axes out again, after another stacked plot changed the width they share. */
	relayout: () => void;
	/** Draws the x range the state's view holds, and fits each y axis to the points in it. */
	rezoom: () => void;
	/** Puts the focus on the plot area. */
	focus: () => void;
	destroy: () => void;
}

const SCALE_KEYS = ["y", "y2"] as const;

export const createPanel = (
	target: HTMLElement,
	options: PanelOptions,
	state: PanelState,
	events: PanelEvents,
): Panel => {
	const { data, lines, styles, axes, size, narrow } = options;
	const axisOf = (line: number) => (styles[line]?.axis ?? 0) as 0 | 1;
	const scaleKey = (line: number) => SCALE_KEYS[axisOf(line)];
	const strokeColor = (line: number) =>
		state.palette().series[(styles[line]?.slot ?? 1) - 1] as string;
	const dimmed = (line: number) => {
		const focus = state.focus();
		return focus !== null && focus !== line;
	};
	// uPlot follows the pixel ratio when a window moves between screens, so read it at each draw.
	const pixelRatio = () => window.devicePixelRatio || 1;
	const font = axisFont(state.palette().code);

	// Scale kinds follow the visible lines; units stay fixed so toggling never relabels an axis.
	const kinds: YScale[] = axes.map(() => ({ kind: "linear" }));
	const steps: number[] = axes.map(() => 0);
	// The points in view, so a zoom fits each y axis to what it shows.
	const domain = (axis: number): [number, number] => {
		const onAxis = axes[axis]?.lines ?? [];
		const visible = onAxis.filter((line) => !state.hidden().has(line));
		const drawn = (visible.length ? visible : onAxis).map(
			(line) => data.lines[line] as PlotLine,
		);
		const view = state.view();
		if (view) {
			const [start, end] = within(x, view);
			const shown = extent(
				drawn.map(({ y, lower, upper }) => ({
					y: y.slice(start, end),
					...(lower && { lower: lower.slice(start, end) }),
					...(upper && { upper: upper.slice(start, end) }),
				})),
			);
			if (shown) {
				return shown;
			}
		}
		return extent(drawn) ?? [0, 1];
	};
	const yScaleOptions = (axis: number): uPlot.Scale => ({
		distr: 100,
		fwd: (value) => transform(kinds[axis] as YScale).fwd(value),
		bwd: (value) => transform(kinds[axis] as YScale).bwd(value),
		range: () => {
			const [min, max] = domain(axis);
			const kind = yScale(options.scale, min, max);
			kinds[axis] = kind;
			return yRange(kind, min, max);
		},
	});
	const yAxis = (axis: number): uPlot.Axis => {
		const { measure } = axes[axis] as PanelAxis;
		return {
			scale: axis === 0 ? "y" : "y2",
			side: axis === 0 ? 3 : 1,
			size: (_, values) =>
				options.axisWidth(axis, values ? axisWidth(values, font) : 0),
			gap: 6,
			font,
			stroke: () => state.palette().faint,
			grid: {
				show: axis === 0,
				stroke: () => state.palette().grid,
				width: 1,
			},
			ticks: { show: false },
			incrs: Y_INCRS,
			splits: (u, _, min, max) => {
				const { factor } = state.tier(measure);
				const count = Math.max(
					3,
					Math.min(5, Math.floor(u.bbox.height / pixelRatio() / TICK_SPACING)),
				);
				const { ticks, step } = yTicks(
					kinds[axis] as YScale,
					min / factor,
					max / factor,
					count,
				);
				steps[axis] = step;
				return ticks.map((tick) => tick * factor);
			},
			// uPlot's default label filter past distr 3 is the log scale's, which blanks most ticks.
			filter: (_, splits) => splits,
			values: (_, splits) => {
				const tier = state.tier(measure);
				return formatTicks(
					splits.map((split) => split / tier.factor),
					steps[axis] ?? 0,
					tier,
				);
			},
		};
	};
	const xLabel =
		options.xAxis === "date"
			? (split: number, increment: number) =>
					increment < MINUTE
						? formatWhenSeconds(split)
						: increment < DAY
							? formatWhen(split)
							: formatDate(split)
			: (split: number) => String(split);
	const xAxisOptions: uPlot.Axis = {
		show: options.showX,
		size: X_AXIS_HEIGHT,
		gap: 6,
		font,
		stroke: () => state.palette().faint,
		grid: { show: false },
		ticks: { show: false },
		space: X_TICK_SPACING,
		// Each label centers on its split, so one too near either edge of the canvas would clip.
		filter: (u, splits, axis, _space, increment) => {
			// uPlot resolves the axis font to the device pixel size it draws, which its types leave out.
			const [drawn] = (u.axes[axis] as uPlot.Axis).font as unknown as [string];
			const width = Math.round(u.width * pixelRatio());
			return splits.map((split) => {
				const center = u.valToPos(split, "x", true);
				const half = labelWidth(xLabel(split, increment), drawn) / 2;
				return center >= half && center + half <= width ? split : null;
			});
		},
		values: (_, splits, _axis, _space, increment) =>
			splits.map((split) => (split == null ? null : xLabel(split, increment))),
		...(options.xAxis !== "date" && {
			incrs: [1, 2, 5, 10, 20, 50, 100, 200, 500, 1000, 2000, 5000, 1e4],
		}),
	};

	const x = data.x as number[];
	const first = x[0] ?? 0;
	const last = x[x.length - 1] ?? 0;
	// uPlot ranges x through this alone, whatever it is asked to set, so a zoom lives in the state's view.
	const xRange = (): [number, number] => {
		const view = state.view();
		if (view) {
			return [view[0], view[1]];
		}
		return first === last ? [first - 1, last + 1] : [first, last];
	};
	const opts: uPlot.Options = {
		width: options.width,
		height: options.height,
		ms: 1,
		// Unaligned, so the lines meet the marks and bands drawn over them at the same y.
		pxAlign: false,
		legend: { show: false },
		select: { show: false, left: 0, top: 0, width: 0, height: 0 },
		padding: [
			10,
			axes.length > 1 ? 0 : narrow ? 10 : 14,
			options.showX ? 0 : 8,
			0,
		],
		cursor: {
			x: true,
			y: false,
			points: { show: false },
			// With no drag, uPlot would still swallow a click it mistakes for the end of one.
			drag: { x: false, y: false, setScale: false, click: () => {} },
			...(options.syncKey !== null && {
				sync: { key: options.syncKey, scales: ["x", null] },
			}),
			// The plot listens for pointer events itself, for touch and the keyboard alike.
			bind: {
				mousedown: () => null,
				mouseup: () => null,
				click: () => null,
				dblclick: () => null,
				mousemove: () => null,
				mouseleave: () => null,
				mouseenter: () => null,
			},
		},
		scales: {
			x: {
				time: options.xAxis === "date",
				range: xRange,
			},
			y: yScaleOptions(0),
			...(axes.length > 1 && { y2: yScaleOptions(1) }),
		},
		axes: [xAxisOptions, ...axes.map((_, axis) => yAxis(axis))],
		series: [
			{},
			...lines.map(
				(line): uPlot.Series => ({
					scale: scaleKey(line),
					stroke: () =>
						dimmed(line) ? state.palette().dim : strokeColor(line),
					...strokeOf(line),
					cap: "round",
					spanGaps: true,
					points: { show: false },
					show: !state.hidden().has(line),
				}),
			),
		],
		hooks: {
			drawAxes: [(u) => drawBand(u), (u) => drawRanges(u)],
			draw: [(u) => drawOverlay(u)],
		},
	};

	function crowded(): boolean {
		return lines.filter((line) => !state.hidden().has(line)).length > CROWDED;
	}

	function strokeOf(
		line: number,
		thick = !crowded(),
	): { width: number; dash: number[] } {
		const focus = state.focus();
		const dotted = lineDash(styles[line] as LineStyle);
		const thin = !thick || (focus !== null && focus !== line);
		const dash = (thin && dotted ? HAIRLINE_DOTS : (dotted ?? [])).map(
			(length) => length * pixelRatio(),
		);
		if (thin) {
			return { width: HAIRLINE, dash };
		}
		return { width: dotted ? DOTTED_WIDTH : WIDTH[size], dash };
	}

	const pointOf = (u: uPlot, line: number, index: number) => {
		const value = data.lines[line]?.y[index];
		return value == null
			? null
			: {
					x: u.valToPos(x[index] ?? 0, "x", true),
					y: u.valToPos(value, scaleKey(line), true),
				};
	};

	const clipToPlot = (u: uPlot, ctx: CanvasRenderingContext2D) => {
		const { left, top, width, height } = u.bbox;
		ctx.beginPath();
		ctx.rect(left, top, width, height);
		ctx.clip();
	};

	const drawBand = (u: uPlot) => {
		const focus = state.focus();
		if (focus === null || !lines.includes(focus) || state.hidden().has(focus)) {
			return;
		}
		const line = data.lines[focus] as PlotLine;
		const geometry = bandGeometry(
			line.through ?? line.y,
			line.lower,
			line.upper,
		);
		if (!geometry.areas.length) {
			return;
		}
		const key = scaleKey(focus);
		const toX = (index: number) => u.valToPos(x[index] ?? 0, "x", true);
		const toY = (value: number) => u.valToPos(value, key, true);
		const top = u.bbox.top;
		const bottom = u.bbox.top + u.bbox.height;
		const run = (indices: number[], column: PlotLine["lower"]) =>
			indices.map((index) => ({
				x: toX(index),
				y: toY(column?.[index] ?? 0),
			}));
		const ctx = u.ctx;
		ctx.save();
		clipToPlot(u, ctx);
		fillAreas(
			ctx,
			geometry.areas.map((area) =>
				area.map((point) => ({
					x: toX(point.index),
					top: point.top === EDGE ? top : toY(point.top),
					bottom: point.bottom === EDGE ? bottom : toY(point.bottom),
				})),
			),
			strokeColor(focus),
		);
		strokeLimits(
			ctx,
			[
				...geometry.lower.map((indices) => run(indices, line.lower)),
				...geometry.upper.map((indices) => run(indices, line.upper)),
			],
			strokeColor(focus),
			pixelRatio(),
		);
		ctx.restore();
	};

	// One per report, so a report's iterations, drawn at its one x, never thin out the marks.
	const pointCount = (line: number, start: number, end: number) => {
		const drawn = data.lines[line];
		const column = drawn?.through ?? drawn?.y ?? [];
		let count = 0;
		for (let index = start; index < end; index++) {
			if (column[index] != null) {
				count++;
			}
		}
		return count;
	};
	const pointCounts = lines.map((line) => pointCount(line, 0, x.length));

	const ranges = lines.map((line) => {
		const drawn = data.lines[line];
		return drawn?.through ? rangeBars(drawn.y, data.report) : [];
	});

	// Under the lines, as the band is: one bar per report from its lowest iteration to its highest.
	const drawRanges = (u: uPlot) => {
		const ctx = u.ctx;
		ctx.save();
		clipToPlot(u, ctx);
		lines.forEach((line, position) => {
			const bars = ranges[position] ?? [];
			if (!bars.length || state.hidden().has(line) || dimmed(line)) {
				return;
			}
			const key = scaleKey(line);
			strokeRanges(
				ctx,
				bars.map(({ index, low, high }) => ({
					x: u.valToPos(x[index] ?? 0, "x", true),
					top: u.valToPos(high, key, true),
					bottom: u.valToPos(low, key, true),
				})),
				strokeColor(line),
				pixelRatio(),
			);
		});
		ctx.restore();
	};

	// Inside save and restore, so uPlot's cache of the context's style stays true.
	const drawOverlay = (u: uPlot) => {
		u.ctx.save();
		drawOverlayStyled(u);
		u.ctx.restore();
	};

	const drawOverlayStyled = (u: uPlot) => {
		const ctx = u.ctx;
		const palette = state.palette();
		const ratio = pixelRatio();
		const focus = state.focus();
		const visible = lines.filter((line) => !state.hidden().has(line));
		// Raise the focused line over the dimmed ones uPlot drew after it.
		if (focus !== null && visible.includes(focus)) {
			// uPlot keeps each series' last stroked path, which its types leave out.
			const series = u.series[lines.indexOf(focus) + 1] as
				| { _paths?: { stroke?: unknown } }
				| undefined;
			const path = series?._paths?.stroke;
			if (path instanceof Path2D) {
				ctx.save();
				clipToPlot(u, ctx);
				ctx.strokeStyle = strokeColor(focus);
				ctx.lineWidth = FOCUSED_WIDTH * ratio;
				ctx.lineCap = "round";
				ctx.lineJoin = "round";
				const dash = lineDash(styles[focus] as LineStyle);
				ctx.setLineDash((dash ?? []).map((length) => length * ratio));
				ctx.stroke(path);
				ctx.restore();
			}
		}
		const markRadius =
			(narrow ? NARROW_MARK_RADIUS : MARK_RADIUS[size]) * ratio;
		// Only the points in view: a zoom spreads them out, and draws none past the plot area.
		const view = state.view();
		const [start, end] = within(x, view);
		for (const line of visible) {
			const count = view
				? pointCount(line, start, end)
				: (pointCounts[lines.indexOf(line)] ?? 0);
			if (dimmed(line) || count * MARK_SPACING * ratio > u.bbox.width) {
				continue;
			}
			const { y } = data.lines[line] as PlotLine;
			for (let index = start; index < Math.min(end, y.length); index++) {
				const point = pointOf(u, line, index);
				if (point) {
					drawMark(
						ctx,
						point,
						styles[line]?.shape ?? "circle",
						markRadius,
						strokeColor(line),
						palette,
						ratio,
					);
				}
			}
		}
		const alertRadius =
			(narrow ? NARROW_ALERT_RADIUS : ALERT_RADIUS[size]) * ratio;
		for (const line of visible) {
			const raised = data.lines[line]?.raised;
			for (const index of data.lines[line]?.alerts ?? []) {
				if (index < start || index >= end) {
					continue;
				}
				const point = pointOf(u, line, index);
				const alert = raised?.find((each) => each.index === index);
				if (point) {
					drawAlert(
						ctx,
						point,
						alertRadius,
						palette,
						alert !== undefined && state.status(alert) !== "active",
					);
				}
			}
		}
	};

	const columns = [
		x,
		...lines.map(
			(line) => data.lines[line]?.through ?? data.lines[line]?.y ?? [],
		),
	];
	const u = new uPlot(opts, columns as uPlot.AlignedData, target);
	const over = u.over;
	over.setAttribute("role", "group");
	over.setAttribute("aria-roledescription", "plot");
	const name = () => {
		const count = lines.filter((line) => !state.hidden().has(line)).length;
		return `${count} ${count === 1 ? "line" : "lines"}, ${options.measures}`;
	};
	over.setAttribute("aria-label", name());
	over.setAttribute("aria-describedby", options.description);
	over.tabIndex = 0;

	const setStrokes = () => {
		const thick = !crowded();
		lines.forEach((line, position) => {
			const series = u.series[position + 1];
			if (series) {
				Object.assign(series, strokeOf(line, thick));
			}
		});
	};

	// The nearest visible line to the pointer at `index`, and its point, among the points that share its x.
	const nearest = (
		index: number,
		top: number,
	): { line: number; point: number } | null => {
		const [start, end] = sameX(x, index);
		let best: { line: number; point: number } | null = null;
		let distance = HOVER_REACH;
		for (const line of lines) {
			if (state.hidden().has(line)) {
				continue;
			}
			const y = data.lines[line]?.y;
			for (let point = start; point < end; point++) {
				const value = y?.[point];
				if (value == null) {
					continue;
				}
				const gap = Math.abs(u.valToPos(value, scaleKey(line)) - top);
				if (gap <= distance) {
					distance = gap;
					best = { line, point };
				}
			}
		}
		return best;
	};

	// uPlot caches the plot area's box and drops it on scroll and resize, so a move reads no layout.
	const local = (event: MouseEvent) => ({
		left: event.clientX - u.rect.left,
		top: event.clientY - u.rect.top,
	});

	// uPlot's types leave out the third argument, which publishes the cursor to synced plots.
	const moveCursor = u.setCursor as (
		position: { left: number; top: number },
		fireHook: boolean,
		publish: boolean,
	) => void;

	const cursorAt = (left: number, top: number): CursorEvent | null => {
		moveCursor({ left, top }, false, true);
		const index = u.cursor.idx;
		if (index == null) {
			return null;
		}
		const near = nearest(index, top);
		return {
			index: near?.point ?? index,
			line: near?.line ?? null,
			x: u.valToPos(x[index] ?? 0, "x"),
		};
	};

	const hide = () => moveCursor({ left: -10, top: -10 }, false, true);

	// A mouse or pen press until it is released or cancelled; once it moves far enough it drags a band from where it pressed.
	let press: { id: number; left: number; top: number } | null = null;
	let dragging = false;
	// The browser sends a click once a drag ends on the plot area, which zooms rather than pins.
	let dropClick = false;
	// Styled here, so the console's one stylesheet, which every page loads, does not carry it.
	const band = document.createElement("div");
	band.style.cssText =
		"position:absolute;top:0;height:100%;border-inline:var(--border-width) solid var(--color-border-accent);background:var(--color-accent-muted);pointer-events:none";
	band.hidden = true;
	over.append(band);
	const inside = (left: number) => Math.max(0, Math.min(u.rect.width, left));

	const pointer =
		(then: (left: number, top: number) => void) => (event: PointerEvent) => {
			if (event.pointerType === "touch" || state.pinned() || dragging) {
				return;
			}
			const { left, top } = local(event);
			then(left, top);
		};
	const onHover = pointer((left, top) => events.hover(cursorAt(left, top)));
	const onMove = (event: PointerEvent) => {
		// A press released off the plot area sends no release here, so a move without the button ends it.
		if (press?.id === event.pointerId && (event.buttons & 1) === 0) {
			endPress();
		}
		if (press?.id === event.pointerId) {
			const { left, top } = local(event);
			if (
				!dragging &&
				Math.hypot(left - press.left, top - press.top) >= DRAG_REACH
			) {
				dragging = true;
				over.setPointerCapture(event.pointerId);
				addEventListener("keydown", onDragKey, true);
				// The readout gives way to the band, a pinned one too.
				hide();
				events.close();
			}
			if (dragging) {
				const from = inside(press.left);
				const to = inside(left);
				band.style.left = `${Math.min(from, to)}px`;
				band.style.width = `${Math.abs(to - from)}px`;
				band.hidden = false;
				return;
			}
		}
		onHover(event);
	};
	const endPress = () => {
		if (press && over.hasPointerCapture(press.id)) {
			over.releasePointerCapture(press.id);
		}
		removeEventListener("keydown", onDragKey, true);
		press = null;
		dragging = false;
		band.hidden = true;
	};
	const onUp = (event: PointerEvent) => {
		if (press?.id !== event.pointerId) {
			return;
		}
		const from = inside(press.left);
		const to = inside(local(event).left);
		const dragged = dragging;
		endPress();
		if (dragged) {
			dropClick = true;
			if (from !== to) {
				const ends = [u.posToVal(from, "x"), u.posToVal(to, "x")];
				events.zoom([Math.min(...ends), Math.max(...ends)]);
			}
		}
	};
	// A drag the browser ends drops the click that may follow it, as Escape does.
	const onCancel = (event: PointerEvent) => {
		if (press?.id === event.pointerId) {
			dropClick = dragging;
			endPress();
		}
	};
	const onDragKey = (event: KeyboardEvent) => {
		if (event.key === "Escape") {
			event.preventDefault();
			event.stopPropagation();
			dropClick = true;
			endPress();
		}
	};
	const onLeave = pointer(() => {
		hide();
		events.hover(null);
	});
	// A browser may send a tap's click as a plain mouse event, so the press tells a tap.
	let touch = false;
	const onDown = (event: PointerEvent) => {
		touch = event.pointerType === "touch";
		dropClick = false;
		// Whatever press came before ended somewhere this plot never heard.
		endPress();
		// A touch never drags, so a finger's swipe still scrolls the page.
		if (!touch && event.isPrimary && event.button === 0) {
			const { left, top } = local(event);
			press = { id: event.pointerId, left, top };
		}
	};
	const onDoubleClick = () => events.unzoom();
	const onClick = (event: MouseEvent) => {
		if (dropClick) {
			dropClick = false;
			return;
		}
		const { left, top } = local(event);
		const cursor = cursorAt(left, top);
		if (cursor) {
			events.click(cursor, touch);
		}
	};
	const onKey = (event: KeyboardEvent) => {
		if (event.key === "Escape") {
			hide();
			events.close();
			return;
		}
		if (event.key === "Enter" || event.key === " ") {
			if (state.index() !== null) {
				event.preventDefault();
				events.toggle();
			}
			return;
		}
		const current = state.index();
		// The keys step through the points in view, so the readout never leaves the plot area.
		const [start, end] = within(x, state.view());
		let index: number;
		switch (event.key) {
			case "ArrowLeft":
				index = current === null ? end - 1 : sameX(x, current)[0] - 1;
				break;
			case "ArrowRight":
				index = current === null ? start : sameX(x, current)[1];
				break;
			case "Home":
				index = start;
				break;
			case "End":
				index = end - 1;
				break;
			default:
				return;
		}
		event.preventDefault();
		if (end > start) {
			pinAt(Math.max(start, Math.min(end - 1, index)));
		}
	};
	const pinAt = (index: number, line: number | null = null) => {
		const left = u.valToPos(x[index] ?? 0, "x");
		moveCursor({ left, top: u.bbox.height / pixelRatio() / 2 }, false, true);
		events.pin({ index, line, x: left });
	};
	const onEnter = () => u.syncRect();
	over.addEventListener("pointerenter", onEnter);
	over.addEventListener("pointermove", onMove);
	over.addEventListener("pointerleave", onLeave);
	over.addEventListener("pointerdown", onDown);
	over.addEventListener("pointerup", onUp);
	over.addEventListener("pointercancel", onCancel);
	over.addEventListener("lostpointercapture", onCancel);
	over.addEventListener("click", onClick);
	over.addEventListener("dblclick", onDoubleClick);
	over.addEventListener("keydown", onKey);

	setStrokes();

	return {
		frame: () => {
			const ratio = pixelRatio();
			return {
				left: u.bbox.left / ratio,
				top: u.bbox.top / ratio,
				width: u.bbox.width / ratio,
			};
		},
		update: () => {
			setStrokes();
			over.setAttribute("aria-label", name());
			const hidden = state.hidden();
			// A batch draws once, rebuilding paths only when a toggle rescales an axis.
			u.batch(() => {
				lines.forEach((line, position) => {
					const show = !hidden.has(line);
					if (u.series[position + 1]?.show !== show) {
						u.setSeries(position + 1, { show });
					}
				});
			});
		},
		pinAt,
		clearCursor: hide,
		setSize: (width, height) => {
			if (width !== u.width || height !== u.height) {
				u.setSize({ width, height });
			}
		},
		relayout: () => u.batch(() => u.redraw(false, true)),
		rezoom: () => {
			const [min, max] = xRange();
			// Setting x sets every auto scale again, and each y range reads the view.
			u.batch(() => u.setScale("x", { min, max }));
		},
		focus: () => over.focus({ preventScroll: true }),
		yOf: (line, index) => {
			const value = data.lines[line]?.y[index];
			return value == null || !lines.includes(line)
				? null
				: u.valToPos(value, scaleKey(line));
		},
		destroy: () => {
			removeEventListener("keydown", onDragKey, true);
			over.removeEventListener("pointerenter", onEnter);
			over.removeEventListener("pointermove", onMove);
			over.removeEventListener("pointerleave", onLeave);
			over.removeEventListener("pointerdown", onDown);
			over.removeEventListener("pointerup", onUp);
			over.removeEventListener("pointercancel", onCancel);
			over.removeEventListener("lostpointercapture", onCancel);
			over.removeEventListener("click", onClick);
			over.removeEventListener("dblclick", onDoubleClick);
			over.removeEventListener("keydown", onKey);
			u.destroy();
		},
	};
};
