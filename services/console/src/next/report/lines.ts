import type {
	JsonConsoleAlert,
	JsonConsoleBenchmark,
	JsonConsoleBranch,
	JsonConsoleMeasure,
	JsonConsolePointReport,
	JsonConsoleReport,
	JsonConsoleSeries,
	JsonConsoleTestbed,
} from "../../types/bencher";
import type { PlotData } from "../plot/types";
import type { Guard } from "../plot/format";
import { guardOf } from "./delta";
import type { ParameterSet } from "./row";
import { lineKey } from "../query/line";

/** A history's columns over its batch's points, with null where it has none. */
export interface AlignedSeries {
	y: readonly (number | null)[];
	baseline?: readonly (number | null)[];
	lower?: readonly (number | null)[];
	upper?: readonly (number | null)[];
	/** Indices into the points of the values that alerted. */
	alerts: readonly number[];
}

/** The points one batch of lines is drawn over. */
interface Points {
	/** Report start times in milliseconds. */
	x: readonly number[];
	/** The report of each point, an index into `reports`. */
	report: readonly number[];
	reports: readonly JsonConsolePointReport[];
}

/** One line of a report, resolved against the tables of the batch it came in. */
export interface ReportLine {
	/** Names the line in the URL, as Explore names it. */
	key: string;
	branch: JsonConsoleBranch;
	testbed: JsonConsoleTestbed;
	benchmark: JsonConsoleBenchmark;
	variant: string;
	parameters: ParameterSet;
	measure: JsonConsoleMeasure;
	metric: string;
	value: number;
	baseline: number | undefined;
	lower_limit: number | undefined;
	upper_limit: number | undefined;
	alert: JsonConsoleAlert | undefined;
	guard: Guard | undefined;
	history: AlignedSeries;
	points: Points;
}

export interface GroupRow {
	name: string;
	lines: number;
	variants: number;
	alerts: number;
	/** The position of the group's first line in the report's drawing order. */
	start: number;
}

export const linesOf = (batch: JsonConsoleReport): ReportLine[] => {
	const points = {
		x: batch.points.x,
		report: batch.points.report,
		reports: batch.reports,
	};
	return batch.lines.flatMap((line) => {
		const benchmark = batch.benchmarks[line.benchmark];
		const variant = batch.variants[line.variant];
		const measure = batch.measures[line.measure];
		if (!(benchmark && variant && measure)) {
			return [];
		}
		const model =
			line.model === undefined ? undefined : batch.models[line.model];
		return [
			{
				key: lineKey({
					branch: batch.branch.uuid,
					testbed: batch.testbed.uuid,
					benchmark: benchmark.uuid,
					parameters: variant.parameters,
					measure: measure.uuid,
					metric: line.metric,
				}),
				branch: batch.branch,
				testbed: batch.testbed,
				benchmark,
				variant: variant.uuid,
				parameters: variant.parameters,
				measure,
				metric: line.metric,
				value: line.value,
				baseline: line.baseline,
				lower_limit: line.lower_limit,
				upper_limit: line.upper_limit,
				alert: line.alert,
				guard: guardOf(model),
				history: alignSeries(line.history, points.x.length),
				points,
			},
		];
	});
};

/** A history over every point of its batch: a thinned one names the positions it kept. */
export const alignSeries = (
	series: JsonConsoleSeries,
	length: number,
): AlignedSeries => {
	const { index } = series;
	const spread = (column: readonly (number | null)[]) => {
		if (!index) {
			return column;
		}
		const aligned = new Array<number | null>(length).fill(null);
		column.forEach((value, position) => {
			const point = index[position];
			if (point !== undefined) {
				aligned[point] = value;
			}
		});
		return aligned;
	};
	return {
		y: spread(series.y),
		...(series.baseline && { baseline: spread(series.baseline) }),
		...(series.lower && { lower: spread(series.lower) }),
		...(series.upper && { upper: spread(series.upper) }),
		alerts: series.alerts.flatMap((alert) => {
			const point = index ? index[alert.index] : alert.index;
			return point === undefined ? [] : [point];
		}),
	};
};

/** The groups of the first batch, each with where its lines start. */
export const groupsOf = (
	first: JsonConsoleReport,
	group: "benchmark" | "measure",
): GroupRow[] => {
	let start = 0;
	return first.groups.map(({ key, lines, variants, alerts }) => {
		const name =
			(group === "measure" ? first.measures[key] : first.benchmarks[key])
				?.name ?? "";
		const row = { name, lines, variants, alerts, start };
		start += lines;
		return row;
	});
};

/** The full plot of one line, from the history the row already holds. */
export const plotOf = (
	line: ReportLine,
	branch: string,
	testbed: string,
): PlotData => ({
	x: line.points.x,
	report: line.points.report,
	reports: line.points.reports,
	measures: [{ name: line.measure.name, units: line.measure.units }],
	lines: [
		{
			id: line.key,
			benchmark: line.benchmark.name,
			parameters: line.parameters,
			measure: 0,
			metric: line.metric,
			branch,
			testbed,
			alerting: line.alert !== undefined,
			...line.history,
		},
	],
});
