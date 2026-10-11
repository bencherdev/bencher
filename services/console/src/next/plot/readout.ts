import { sameX } from "./columns";
import {
	type Delta,
	formatDelta,
	formatValue,
	formatWhen,
	type Guard,
} from "./format";
import type { PlotData, PlotLine, PlotXAxis } from "./types";
import type { UnitScale } from "./units";

const READOUT_ROWS = 6;

const NO_SCALE: UnitScale = { factor: 1, symbol: "" };

interface ReadoutRow {
	/** An index into the data's lines. */
	line: number;
	value: string;
	/** The report the point came from, an index into the data's reports. */
	report: number;
	focused: boolean;
	delta?: Delta | null;
	limit?: string;
}

export interface Readout {
	when: string;
	report: number;
	rows: ReadoutRow[];
	/** The visible lines with a point here that the rows leave out. */
	more: number;
}

/** What the plot reads out at `index` for the visible lines in `order`, the focused line first. */
export const readout = (
	data: PlotData,
	index: number,
	order: readonly number[],
	focus: number | null,
	scales: readonly UnitScale[],
	xAxis: PlotXAxis,
	timeZone?: string,
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
		const point = line && lastPoint(line, start, end);
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

const lastPoint = (
	line: PlotLine,
	start: number,
	end: number,
): number | undefined => {
	for (let index = end - 1; index >= start; index--) {
		if (line.y[index] != null) {
			return index;
		}
	}
	return undefined;
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
