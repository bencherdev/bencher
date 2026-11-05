import { LINE_CAP } from "../explore/layout";
import { lineKey } from "./line";
import { canonicalParameters } from "./parameters";
import {
	type BranchEntry,
	type ExploreQuery,
	MAX_ENTRIES,
	type Parameters,
	type QueryWindow,
	type TestbedEntry,
	blankQuery,
	distinct,
	encodeQuery,
	itself,
	narrowingSets,
} from "./query";

/** A line picked on another page, such as a report's row. */
export interface SelectedLine {
	readonly branch: BranchEntry;
	readonly testbed: TestbedEntry;
	readonly benchmark: string;
	readonly parameters: Parameters;
	readonly measure: string;
	readonly metric: string;
	readonly alerting?: boolean;
}

export interface SelectionContext {
	readonly window?: QueryWindow;
	readonly report?: string;
}

/** Explore's query string for these lines, as many as it opens: extras of the boxes' cross product start hidden. */
export const exploreSearch = (
	lines: readonly SelectedLine[],
	context: SelectionContext = {},
): string => encodeQuery(selectionQuery(keptLines(lines), context));

/** The lines of these that Explore opens: those in the first values of every box it reads, up to the lines a plot draws. */
export const keptLines = <T extends SelectedLine>(lines: readonly T[]): T[] => {
	const first = <V>(values: V[], id: (value: V) => string) =>
		new Set(distinct(values, id, MAX_ENTRIES).map(id));
	const branches = first(
		lines.map(({ branch }) => branch.uuid),
		itself,
	);
	const testbeds = first(
		lines.map(({ testbed }) => testbed.uuid),
		itself,
	);
	const benchmarks = first(
		lines.map(({ benchmark }) => benchmark),
		itself,
	);
	const measures = first(
		lines.map(({ measure }) => measure),
		itself,
	);
	const metrics = first(
		lines.map(({ metric }) => metric),
		itself,
	);
	const boxed = lines.filter(
		(line) =>
			branches.has(line.branch.uuid) &&
			testbeds.has(line.testbed.uuid) &&
			benchmarks.has(line.benchmark) &&
			measures.has(line.measure) &&
			metrics.has(line.metric),
	);
	const drawn = new Set(distinct(boxed.map(keyOf), itself, LINE_CAP));
	return boxed.filter((line) => drawn.has(keyOf(line)));
};

const selectionQuery = (
	lines: readonly SelectedLine[],
	{ window, report }: SelectionContext,
): ExploreQuery => {
	const keys = distinct(lines.map(keyOf), itself);
	const alerting = lines.find((line) => line.alerting);
	const [lone] = keys.length === 1 ? keys : [];
	const focus = alerting === undefined ? lone : keyOf(alerting);
	const sets = narrowingSets(
		distinct(
			lines.map(({ parameters }) => parameters),
			canonicalParameters,
		),
	);
	return {
		...blankQuery(),
		branches: distinct(
			lines.map(({ branch }) => branch),
			({ uuid }) => uuid,
		),
		testbeds: distinct(
			lines.map(({ testbed }) => testbed),
			({ uuid }) => uuid,
		),
		benchmarks: distinct(
			lines.map(({ benchmark }) => benchmark),
			itself,
		),
		sets: sets.length > MAX_ENTRIES ? shared(sets) : sets,
		measures: distinct(
			lines.map(({ measure }) => measure),
			itself,
		),
		metrics: distinct(
			lines.map(({ metric }) => metric),
			itself,
		),
		only: keys,
		...(focus === undefined ? {} : { focus }),
		...(window === undefined ? {} : { window }),
		...(report === undefined ? {} : { report }),
	};
};

const keyOf = ({ branch, testbed, ...line }: SelectedLine): string =>
	lineKey({ ...line, branch: branch.uuid, testbed: testbed.uuid });

/** One set of the tags every variant carries, or none (every variant) when they share nothing. */
const shared = (sets: readonly Parameters[]): Parameters[] => {
	const [first = {}, ...rest] = sets;
	const tags = Object.entries(first).filter(([key, value]) =>
		rest.every((set) => set[key] === value),
	);
	return tags.length > 0 ? [Object.fromEntries(tags)] : [];
};
