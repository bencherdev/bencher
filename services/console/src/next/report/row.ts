import { formatMeasure, type Guard } from "../plot/format";
import { restsOf, varyingKeys } from "../plot/distinct";

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

interface NamedLine {
	benchmark: string;
	parameters: ParameterSet;
	measure: string;
	metric: string;
}

/**
 * Each line's name for the controls that act on it alone, among the lines of
 * its list: what its benchmark adds to the one they share, or its last segment
 * when it is alone or they share it whole, and the parameters that vary; then
 * the measure, and the metric where the list shows it, where they tell apart
 * two lines of one name.
 */
export const shortNames = (
	lines: readonly NamedLine[],
	manyMetrics: boolean,
): string[] => {
	const rests = restsOf(lines.map(({ benchmark }) => benchmark));
	const varying = new Set(varyingKeys(lines));
	const bases = lines.map((line, index) =>
		[
			rests[index] ?? line.benchmark,
			...parameterEntries(line.parameters)
				.filter(([key]) => varying.has(key))
				.map(([key, value]) => `${key}=${value}`),
		].join(" "),
	);
	// What else the lines that share a name differ in.
	const differ = (part: (line: NamedLine) => string) => {
		const seen = new Map<string, Set<string>>();
		lines.forEach((line, index) => {
			const base = bases[index] ?? "";
			seen.set(base, (seen.get(base) ?? new Set()).add(part(line)));
		});
		return (index: number) => (seen.get(bases[index] ?? "")?.size ?? 0) > 1;
	};
	const measure = differ(({ measure }) => measure);
	const metric = differ(({ metric }) => metric);
	return lines.map((line, index) =>
		[
			bases[index] ?? "",
			...(measure(index) ? [line.measure] : []),
			...(manyMetrics && metric(index) ? [line.metric] : []),
		].join(" "),
	);
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
