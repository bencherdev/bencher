import { leafOf } from "../leaf";
import { distinct, varyingKeys } from "./distinct";
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

const tag = (key: string, value: PlotParameterValue): string =>
	`${key}=${String(value)}`;

/** The parameters every line carries with one value, which key entries leave out. */
export const constantParameters = (lines: readonly PlotLine[]): string[] => {
	const [first] = lines;
	if (!first || lines.length < 2) {
		return [];
	}
	// A key some line lacks varies, so the first line's other keys are on every line.
	const varying = new Set(varyingKeys(lines));
	return Object.entries(first.parameters).flatMap(([key, value]) =>
		varying.has(key) ? [] : [tag(key, value)],
	);
};

export interface LineName {
	/** What the line's benchmark adds to the benchmark all lines share. */
	rest: string;
	/** The parameters that vary across the lines, as `key=value`, or the value alone when the key heads the key. */
	tags: string[];
	/** The metric, measure, branch, and testbed, each only when the lines differ in it. */
	detail: string[];
	text: string;
}

export interface LineNames {
	names: LineName[];
	/** What every line shares: the benchmark, the parameters with one value, and the key of the one parameter that varies. */
	shared: { benchmark: string; parameters: string[] };
}

export const lineNames = (
	lines: readonly PlotLine[],
	measures: readonly PlotMeasure[],
): LineNames => {
	const varying = new Set(varyingKeys(lines));
	// The one key that varies heads the key, so each entry shows its value alone, when every line has one.
	const [first] = varying;
	const only =
		varying.size === 1 &&
		lines.every((line) => first !== undefined && first in line.parameters)
			? first
			: undefined;
	const differ = (part: (line: PlotLine) => string | number) =>
		new Set(lines.map(part)).size > 1;
	const metric = differ(({ metric }) => metric);
	const measure = differ(({ measure }) => measure);
	const branch = differ(({ branch }) => branch);
	const testbed = differ(({ testbed }) => testbed);
	const { prefix, rests } = distinct(lines.map(({ benchmark }) => benchmark));
	const names = lines.map((line, index) => {
		const given = rests[index] ?? line.benchmark;
		const tags = Object.entries(line.parameters)
			.filter(([key]) => varying.has(key))
			.map(([key, value]) => (key === only ? String(value) : tag(key, value)));
		const detail = [
			metric ? line.metric : undefined,
			measure ? measures[line.measure]?.name : undefined,
			branch ? line.branch : undefined,
			testbed ? line.testbed : undefined,
		].filter((part): part is string => part !== undefined);
		// A line the others tell apart by what it lacks is named by its benchmark's last segment.
		const rest =
			given === "" && tags.length === 0 && detail.length === 0
				? leafOf(line.benchmark)
				: given;
		return {
			rest,
			tags,
			detail,
			text: [rest, ...tags, ...detail].filter((part) => part !== "").join(" "),
		};
	});
	return {
		names,
		shared: {
			benchmark: prefix.replace(/(::|\/)$/, ""),
			parameters: [
				...constantParameters(lines),
				...(only === undefined ? [] : [`${only}=`]),
			],
		},
	};
};
