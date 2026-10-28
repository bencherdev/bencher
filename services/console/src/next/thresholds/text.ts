import type { ModelTest } from "../../types/bencher";
import { formatCount } from "../plot/count";

/** A threshold model as the API names its fields, its test by name. */
export interface ModelLike {
	test: `${ModelTest}`;
	min_sample_size?: number | null | undefined;
	max_sample_size?: number | null | undefined;
	window?: number | null | undefined;
	lower_boundary?: number | null | undefined;
	upper_boundary?: number | null | undefined;
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

/** Every field of the model by its API name, with its value as text or undefined when absent, as some endpoints send null. */
export const modelRows = (model: ModelLike): [Field, string | undefined][] =>
	FIELDS.map((field) => {
		const value = model[field];
		return [
			field,
			value == null
				? undefined
				: field === "window"
					? durationText(value)
					: field.endsWith("_boundary")
						? boundaryText(value)
						: formatCount(value),
		];
	});

/** A boundary to the twelve significant digits a person sets one in, without the noise of binary fractions. */
export const boundaryText = (boundary: number) =>
	String(Number(boundary.toPrecision(12))).replace("e+", "e");

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
	return `${formatCount(count)} ${unit}${count === 1 ? "" : "s"}`;
};
