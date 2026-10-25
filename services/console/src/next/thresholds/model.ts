import type { ParameterSet } from "../report/row";
import { parameterTag } from "../tag";

/** Each set of a parameters filter as its tags, in the API's key order. */
export const filterSets = (sets: readonly ParameterSet[] | undefined) =>
	(sets ?? []).map((set) =>
		// The API's key order, as `parameterEntries` sorts; that module stays out of the list's code.
		Object.entries(set)
			.sort(([a], [b]) => (a < b ? -1 : 1))
			.map(([key, value]) => parameterTag(key, value)),
	);

/** What a parameters filter lets through, in a few words. */
export const filterSummary = (sets: readonly ParameterSet[] | undefined) => {
	const count = sets?.length ?? 0;
	return count === 0
		? "every variant"
		: `${count} parameter set${count === 1 ? "" : "s"}`;
};

/** A day, with its year when that is not this year. */
export const dayText = (time: number, now: number, timeZone?: string) =>
	dateFormat(yearOf(time, timeZone) !== yearOf(now, timeZone), timeZone).format(
		time,
	);

const dateFormat = (withYear: boolean, timeZone?: string) =>
	new Intl.DateTimeFormat("en-US", {
		month: "short",
		day: "numeric",
		...(withYear ? { year: "numeric" } : {}),
		timeZone,
	});

const yearOf = (time: number, timeZone?: string) =>
	new Intl.DateTimeFormat("en-US", { year: "numeric", timeZone }).format(time);

/** When a model held: since it was set, or from then until it was replaced. */
export const historySpan = (
	model: { created: number; replaced?: number | undefined },
	now: number,
	timeZone?: string,
) => {
	const from = dayText(model.created, now, timeZone);
	return model.replaced === undefined
		? `since ${from}`
		: `${from} to ${dayText(model.replaced, now, timeZone)}`;
};

interface Dimension {
	name: string;
	archived?: number | undefined;
}

/** The dimension whose archiving archived the threshold: the first archived, if any is. */
export const archivedBy = (dimensions: {
	branch: Dimension;
	testbed: Dimension;
	measure: Dimension;
}) => {
	let first: { kind: string; name: string; archived: number } | undefined;
	for (const kind of ["branch", "testbed", "measure"] as const) {
		const { name, archived } = dimensions[kind];
		if (
			archived !== undefined &&
			(first === undefined || archived < first.archived)
		) {
			first = { kind, name, archived };
		}
	}
	return first;
};
