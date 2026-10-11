import type { ModelTest } from "../../types/bencher";
import type { ParameterSet } from "../report/row";

/** A threshold model as the API names its fields, its test by name. */
export interface ModelLike {
	test: `${ModelTest}`;
	min_sample_size?: number | undefined;
	max_sample_size?: number | undefined;
	window?: number | undefined;
	lower_boundary?: number | undefined;
	upper_boundary?: number | undefined;
}

type Field = Exclude<keyof ModelLike, "test">;

// The board's order.
const FIELDS: readonly Field[] = [
	"upper_boundary",
	"lower_boundary",
	"max_sample_size",
	"min_sample_size",
	"window",
];

/** Every field of the model by its API name, with its value as text or undefined when absent. */
export const modelRows = (model: ModelLike): [Field, string | undefined][] =>
	FIELDS.map((field) => {
		const value = model[field];
		return [
			field,
			value === undefined
				? undefined
				: field === "window"
					? durationText(value)
					: String(value),
		];
	});

/** The model on one line: its test, then each field it has. */
export const modelText = (model: ModelLike | undefined) =>
	model
		? [
				model.test,
				...modelRows(model).flatMap(([field, value]) =>
					value === undefined ? [] : [`${field} ${value}`],
				),
			].join(" · ")
		: "no model";

const UNITS = [
	["week", 7 * 24 * 60 * 60],
	["day", 24 * 60 * 60],
	["hour", 60 * 60],
	["minute", 60],
] as const;

/** A window in seconds, in the largest unit it is a whole number of. */
export const durationText = (seconds: number) => {
	const [unit, size] = UNITS.find(([, size]) => seconds % size === 0) ?? [
		"second",
		1,
	];
	const count = seconds / size;
	return `${count} ${unit}${count === 1 ? "" : "s"}`;
};

/** Each set of a parameters filter as its tags, in the API's key order. */
export const filterSets = (sets: readonly ParameterSet[] | undefined) =>
	(sets ?? []).map((set) =>
		// The API's key order, as `parameterEntries` sorts; that module stays out of the list's code.
		Object.entries(set)
			.sort(([a], [b]) => (a < b ? -1 : 1))
			.map(([key, value]) => `${key}=${value}`),
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
