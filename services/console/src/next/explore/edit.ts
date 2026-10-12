import { canonicalParameters } from "../query/parameters";
import {
	type ExploreQuery,
	MAX_ENTRIES,
	type Parameters,
} from "../query/query";

/** What each box adds: a UUID, a parameter set, or a metric name. */
interface BoxValues {
	branches: string;
	testbeds: string;
	benchmarks: string;
	sets: Parameters;
	measures: string;
	metrics: string;
}

export type Box = keyof BoxValues;

type Entry = string | Parameters | { uuid: string };

/** The query with `value` added to the end of `box`, unless the box has it or is full. */
export const withValue = <B extends Box>(
	query: ExploreQuery,
	box: B,
	value: BoxValues[B],
): ExploreQuery => {
	const entries: readonly Entry[] = query[box];
	const entry: Entry =
		box === "branches" || box === "testbeds"
			? { uuid: value as string }
			: value;
	if (
		entries.length >= MAX_ENTRIES ||
		entries.some((other) => identity(box, other) === identity(box, entry))
	) {
		return query;
	}
	return { ...query, [box]: [...entries, entry] };
};

const identity = (box: Box, entry: Entry): string => {
	if (box === "branches" || box === "testbeds") {
		return (entry as { uuid: string }).uuid;
	}
	return box === "sets"
		? canonicalParameters(entry as Parameters)
		: (entry as string);
};

export const withoutValue = (
	query: ExploreQuery,
	box: Box,
	index: number,
): ExploreQuery => ({
	...query,
	[box]: (query[box] as readonly Entry[]).filter((_, at) => at !== index),
});

/** The six boxes, in the ruled order. */
export const BOXES = [
	"branches",
	"testbeds",
	"benchmarks",
	"sets",
	"measures",
	"metrics",
] as const satisfies readonly Box[];

export const hasValues = (query: ExploreQuery): boolean =>
	BOXES.some((box) => query[box].length > 0);

/** The query with every box emptied, and the lines, report, and pin they named let go. */
export const withoutValues = ({
	only: _only,
	focus: _focus,
	report: _report,
	plot: _plot,
	...query
}: ExploreQuery): ExploreQuery => ({
	...query,
	...Object.fromEntries(BOXES.map((box) => [box, []])),
	hide: [],
});
