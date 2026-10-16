import { sameX } from "./columns";
import {
	type Delta,
	formatDelta,
	formatValue,
	formatWhen,
	type Guard,
} from "./format";
import type {
	PlotAlert,
	PlotData,
	PlotLine,
	PlotParameterValue,
	PlotXAxis,
} from "./types";
import type { UnitScale } from "./units";

const READOUT_ROWS = 6;

/** A variant's parameters as tags, in the API's key order. */
export const tagsOf = (
	parameters: Readonly<Record<string, PlotParameterValue>>,
) =>
	Object.keys(parameters)
		.sort()
		.map((key) => `${key}=${String(parameters[key])}`);

// A threshold with no metric checks the conventional name.
const VALUE = "value";

/** A line in full, as a control that acts on it alone names it: its metric only when not the conventional one. */
export const fullName = (
	benchmark: string,
	parameters: Readonly<Record<string, PlotParameterValue>>,
	measure: string | undefined,
	metric: string,
) =>
	[
		benchmark,
		...tagsOf(parameters),
		measure,
		...(metric === VALUE ? [] : [metric]),
	].join(" ");

const NO_SCALE: UnitScale = { factor: 1, symbol: "" };

/** One iteration of the focused line's report. */
interface ReadoutIteration {
	/** Counting from 1. */
	iteration: number;
	value: string;
	/** The one the row's value, delta, limit, and alert read. */
	emphasized: boolean;
	alerted: boolean;
}

interface ReadoutRow {
	/** An index into the data's lines. */
	line: number;
	/** The data index the row reads. */
	point: number;
	value: string;
	/** The report the point came from, an index into the data's reports. */
	report: number;
	focused: boolean;
	delta?: Delta | null;
	limit?: string;
	/** The alert raised at the focused line's point. */
	alert?: PlotAlert;
	/** The focused or only line's iterations at its report, where it ran several or one is chosen. */
	iterations?: ReadoutIteration[];
}

export interface Readout {
	when: string;
	report: number;
	rows: ReadoutRow[];
	/** The visible lines with a point here that the rows leave out. */
	more: number;
}

/**
 * What the plot reads out at `index` for the visible lines in `order`, the
 * focused line first. Each line reads the point it runs through, except the
 * focused line under the cursor, which reads the point the cursor is on.
 */
export const readout = (
	data: PlotData,
	index: number,
	order: readonly number[],
	focus: number | null,
	scales: readonly UnitScale[],
	xAxis: PlotXAxis,
	timeZone?: string,
	cursor: { line: number | null; chosen?: number | undefined } = {
		line: null,
	},
): Readout => {
	const [start, end] = sameX(data.x, index);
	const lines =
		focus !== null && order.includes(focus)
			? [focus, ...order.filter((line) => line !== focus)]
			: order;
	const rows: ReadoutRow[] = [];
	let count = 0;
	for (const lineIndex of lines) {
		const line = data.lines[lineIndex];
		const point =
			line &&
			(lineIndex === focus && cursor.line === focus && line.y[index] != null
				? index
				: shownPoint(line, start, end));
		if (!line || point === undefined) {
			continue;
		}
		count++;
		if (rows.length === READOUT_ROWS) {
			continue;
		}
		const scale = scales[line.measure] ?? NO_SCALE;
		const row: ReadoutRow = {
			line: lineIndex,
			point,
			value: formatValue(line.y[point] ?? 0, scale),
			report: data.report[point] ?? 0,
			focused: lineIndex === focus,
		};
		if (row.focused) {
			row.delta = deltaAt(line, point);
			const limit = limitAt(line, point, scale);
			if (limit) {
				row.limit = limit;
			}
			const alert = line.raised?.find(({ index }) => index === point);
			if (alert) {
				row.alert = alert;
			}
		}
		// The focused line, or the only one, lists its report's iterations.
		const iterations =
			(row.focused || lines.length === 1) &&
			iterationsAt(data, line, point, scale, cursor.chosen);
		if (iterations) {
			row.iterations = iterations;
		}
		rows.push(row);
	}
	const report = rows[0]?.report ?? data.report[index] ?? 0;
	return {
		when:
			xAxis === "date"
				? formatWhen(data.x[index] ?? 0, timeZone)
				: `Version ${data.reports[report]?.version}`,
		report,
		rows,
		more: count - rows.length,
	};
};

/** The last point among those that share the x that the line runs through. */
const shownPoint = (
	line: PlotLine,
	start: number,
	end: number,
): number | undefined => {
	const values = line.through ?? line.y;
	for (let index = end - 1; index >= start; index--) {
		if (values[index] != null) {
			return index;
		}
	}
	return undefined;
};

/** Every iteration of the report at `point` the line has a value at, in order. */
const iterationsAt = (
	data: PlotData,
	line: PlotLine,
	point: number,
	scale: UnitScale,
	chosen: number | undefined,
): ReadoutIteration[] | undefined => {
	const { iteration, report } = data;
	const [start, end] = sameX(data.x, point);
	const points = Array.from(
		{ length: end - start },
		(_, at) => start + at,
	).filter((index) => report[index] === report[point] && line.y[index] != null);
	return iteration && (points.length > 1 || chosen !== undefined)
		? points.map((index) => ({
				iteration: (iteration[index] ?? 0) + 1,
				value: formatValue(line.y[index] ?? 0, scale),
				emphasized: index === point,
				alerted: line.alerts.includes(index),
			}))
		: undefined;
};

const guardAt = (line: PlotLine, point: number): Guard | undefined => {
	const lower = line.lower?.[point] != null;
	const upper = line.upper?.[point] != null;
	if (lower && upper) {
		return "both";
	}
	return upper ? "upper" : lower ? "lower" : undefined;
};

const deltaAt = (line: PlotLine, point: number): Delta | null => {
	const value = line.y[point];
	const baseline = line.baseline?.[point];
	const guard = guardAt(line, point);
	return value == null || baseline == null || !guard
		? null
		: formatDelta(value, baseline, guard);
};

const limitAt = (
	line: PlotLine,
	point: number,
	scale: UnitScale,
): string | undefined => {
	if (!line.lower && !line.upper) {
		return "no threshold";
	}
	const lower = line.lower?.[point];
	const upper = line.upper?.[point];
	if (lower != null && upper != null) {
		return `limits ${formatValue(lower, scale)} to ${formatValue(upper, scale)}`;
	}
	const limit = upper ?? lower;
	return limit == null ? undefined : `limit ${formatValue(limit, scale)}`;
};

const GAP = 12;
const EDGE = 4;

/** Where a readout `width` wide starts beside a cursor at `x`: right of it if it fits, else left, else as near as `room` allows. */
export const readoutLeft = (x: number, width: number, room: number): number => {
	if (x + GAP + width <= room - EDGE) {
		return x + GAP;
	}
	if (x - GAP - width >= EDGE) {
		return x - GAP - width;
	}
	return Math.max(EDGE, Math.min(room - EDGE - width, x - width / 2));
};

/** Where the readout reads: a data index of one panel, the x it sits at, the line it is on if any, and whether a click, tap, or key pinned it there. */
export interface Cursor {
	index: number;
	panel: number;
	x: number;
	line: number | null;
	pinned: boolean;
}

/** The cursor after a click or a tap at `at`: one on the pinned point, at the same x of the same panel and on its line at its iteration or on none, unpins it, to follow a mouse or close for a tap. */
export const clicked = (
	current: Cursor | null,
	at: Omit<Cursor, "pinned">,
	xs: readonly number[],
	touch: boolean,
	iteration?: readonly number[],
): Cursor | null => {
	const onPin =
		current?.pinned &&
		current.panel === at.panel &&
		xs[current.index] === xs[at.index] &&
		(at.line === null ||
			current.line === null ||
			(at.line === current.line &&
				(iteration?.[at.index] ?? 0) === (iteration?.[current.index] ?? 0)));
	if (!onPin) {
		return { ...at, pinned: true };
	}
	return touch ? null : { ...at, pinned: false };
};
