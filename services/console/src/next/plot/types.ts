// The plot's input mirrors the console's plot query: columns aligned on one
// shared x, so uPlot takes them as they arrive.

export type PlotParameterValue = string | number | boolean;

interface PlotReport {
	uuid: string;
	version: number;
	hash?: string;
}

export interface PlotMeasure {
	name: string;
	units: string;
}

export interface PlotLine {
	/** Stable across requests; the query's hidden and focused lines name it. */
	id: string;
	benchmark: string;
	parameters: Readonly<Record<string, PlotParameterValue>>;
	/** An index into `measures`, which picks the line's y axis: the first measure draws on the left axis or the top plot. */
	measure: number;
	metric: string;
	branch: string;
	testbed: string;
	alerting: boolean;
	/** The threshold model behind the line's limits, as the boundary names it. */
	model?: string;
	/** Aligned to `x`, with null where the line has no point. */
	y: readonly (number | null)[];
	baseline?: readonly (number | null)[];
	lower?: readonly (number | null)[];
	upper?: readonly (number | null)[];
	/** Indices into `x` of the points that alerted. */
	alerts: readonly number[];
}

export interface PlotData {
	/** Report start times in milliseconds, ascending. */
	x: readonly number[];
	/** The report of each point, an index into `reports`. */
	report: readonly number[];
	reports: readonly PlotReport[];
	measures: readonly PlotMeasure[];
	lines: readonly PlotLine[];
}

export type PlotXAxis = "date" | "version";

export type PlotScale = "auto" | "linear" | "log";

export type PlotLayout = "dual" | "stacked";

/** Explore and the public plot, a Plots tile, or a row expanded in place. */
export type PlotSize = "full" | "tile" | "row";
