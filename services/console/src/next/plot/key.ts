import type { PlotLine, PlotMeasure, PlotParameterValue } from "./types";

/** Line indices in key order: alerting lines first, then the given order. */
export const keyOrder = (lines: readonly PlotLine[]): number[] => {
	const indices = lines.map((_, index) => index);
	return [
		...indices.filter((index) => lines[index]?.alerting),
		...indices.filter((index) => !lines[index]?.alerting),
	];
};

/** A parameter set's identity, whatever order its keys arrive in. */
export const parameterKey = (
	parameters: Readonly<Record<string, PlotParameterValue>>,
): string =>
	JSON.stringify(
		Object.keys(parameters)
			.sort()
			.map((key) => [key, parameters[key]]),
	);

const valueKey = (value: PlotParameterValue | undefined): string =>
	value === undefined ? "" : JSON.stringify(value);

const keysOf = (lines: readonly PlotLine[]): string[] => {
	const keys = new Set<string>();
	for (const line of lines) {
		for (const key of Object.keys(line.parameters)) {
			keys.add(key);
		}
	}
	return [...keys];
};

/** The parameter keys whose values differ across the lines, a missing key counting as a value. */
export const varyingKeys = (lines: readonly PlotLine[]): string[] =>
	keysOf(lines).filter(
		(key) =>
			new Set(lines.map((line) => valueKey(line.parameters[key]))).size > 1,
	);

const tag = (key: string, value: PlotParameterValue): string =>
	`${key}=${String(value)}`;

/** The parameters every line carries with one value, which key entries leave out. */
export const constantParameters = (lines: readonly PlotLine[]): string[] => {
	const [first] = lines;
	if (!first || lines.length < 2) {
		return [];
	}
	return keysOf(lines).flatMap((key) => {
		const value = first.parameters[key];
		return value !== undefined &&
			lines.every((line) => valueKey(line.parameters[key]) === valueKey(value))
			? [tag(key, value)]
			: [];
	});
};

export interface LineName {
	benchmark: string;
	/** The parameters that vary across the lines, as `key=value`. */
	tags: string[];
	/** The metric, measure, branch, and testbed, each only when the lines differ in it. */
	detail: string[];
	text: string;
}

export const lineNames = (
	lines: readonly PlotLine[],
	measures: readonly PlotMeasure[],
): LineName[] => {
	const varying = new Set(varyingKeys(lines));
	const differ = (part: (line: PlotLine) => string | number) =>
		new Set(lines.map(part)).size > 1;
	const metric = differ(({ metric }) => metric);
	const measure = differ(({ measure }) => measure);
	const branch = differ(({ branch }) => branch);
	const testbed = differ(({ testbed }) => testbed);
	return lines.map((line) => {
		const tags = Object.entries(line.parameters)
			.filter(([key]) => varying.has(key))
			.map(([key, value]) => tag(key, value));
		const detail = [
			metric ? line.metric : undefined,
			measure ? measures[line.measure]?.name : undefined,
			branch ? line.branch : undefined,
			testbed ? line.testbed : undefined,
		].filter((part): part is string => part !== undefined);
		return {
			benchmark: line.benchmark,
			tags,
			detail,
			text: [line.benchmark, ...tags, ...detail].join(" "),
		};
	});
};
