import type { Guard } from "../plot/format";

export type ParameterSet = Readonly<Record<string, string | number | boolean>>;

/** The plot's unit formatting, which the row borrows so a row and its plot print numbers alike. */
export interface UnitsPort {
	unitScale: (min: number, units: string) => { factor: number; symbol: string };
	formatValue: (
		value: number,
		scale: { factor: number; symbol: string },
	) => string;
}

/** A variant's parameters in the API's key order, by UTF-16 code unit, which object key order is not. */
export const parameterEntries = (parameters: ParameterSet) =>
	Object.entries(parameters).sort(([a], [b]) => (a < b ? -1 : 1));

export const lineLabel = (
	line: {
		benchmark: string;
		parameters: ParameterSet;
		measure: string;
		metric: string;
	},
	manyMetrics: boolean,
) => {
	const tags = parameterEntries(line.parameters).map(
		([key, value]) => `${key}=${value}`,
	);
	const metric = manyMetrics ? line.metric : null;
	return {
		tags,
		metric,
		name: [
			line.benchmark,
			...tags,
			line.measure,
			...(metric ? [metric] : []),
		].join(" "),
	};
};

interface RowLine {
	value: number;
	baseline?: number | null | undefined;
	lower_limit?: number | null | undefined;
	upper_limit?: number | null | undefined;
	alert?: { limit: "lower" | "upper" } | null | undefined;
}

/** A row's value and limit, in one unit fit to the smaller so neither prints under one. */
export const rowNumbers = (
	line: RowLine,
	guard: Guard | undefined,
	units: string,
	port: UnitsPort,
) => {
	const limit = guard ? limitOf(line, guard) : null;
	const scale = port.unitScale(
		Math.min(line.value, limit ?? line.value),
		units,
	);
	return {
		value: port.formatValue(line.value, scale),
		limit: limit == null ? null : port.formatValue(limit, scale),
	};
};

// A two-sided threshold shows the side it alerted on, else the side the value moved toward.
const limitOf = (line: RowLine, guard: Guard) => {
	const side =
		guard === "both"
			? (line.alert?.limit ??
				(line.baseline != null && line.value < line.baseline
					? "lower"
					: "upper"))
			: guard;
	return side === "lower" ? line.lower_limit : line.upper_limit;
};
