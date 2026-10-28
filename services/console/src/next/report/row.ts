import { formatMeasure, type Guard } from "../plot/format";

export type ParameterSet = Readonly<Record<string, string | number | boolean>>;

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

/** Which iteration a row shows, of how many, when its line ran more than once. */
export const iterationTag = (line: {
	iteration?: number | undefined;
	iterations?: number | undefined;
}) =>
	(line.iterations ?? 1) > 1
		? `iteration ${(line.iteration ?? 0) + 1} of ${line.iterations}`
		: undefined;

interface RowLine {
	value: number;
	baseline?: number | null | undefined;
	lower_limit?: number | null | undefined;
	upper_limit?: number | null | undefined;
	alert?: { limit: "lower" | "upper" } | null | undefined;
}

/** A row's value and the limit it is held to, as the plot prints them. */
export const rowNumbers = (
	line: RowLine,
	guard: Guard | undefined,
	units: string,
) => {
	const limit = guard ? limitOf(line, guard) : null;
	return {
		value: formatMeasure(line.value, units),
		limit: limit == null ? null : formatMeasure(limit, units),
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
