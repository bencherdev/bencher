import type { Parameters } from "../query/query";

/** Whether the variant carries every tag of the set, as the API filters; values keep their JSON type. */
export const matchesSet = (variant: Parameters, set: Parameters): boolean =>
	Object.entries(set).every(([key, value]) => variant[key] === value);

/** One row of the parameters box. */
export interface ParameterRow {
	/** The variants the row's set matches on its own. */
	readonly variants: number;
	/** A wider row whose every tag this row also holds, so this row adds no variants. */
	readonly coveredBy?: number;
}

export const parameterRows = (
	sets: readonly Parameters[],
	variants: readonly Parameters[],
): ParameterRow[] =>
	sets.map((set, index) => {
		const count = variants.filter((variant) => matchesSet(variant, set)).length;
		// Of two equal sets, the later one is the copy.
		const coveredBy = sets.findIndex(
			(other, at) =>
				at !== index &&
				matchesSet(set, other) &&
				(size(other) < size(set) || at < index),
		);
		return coveredBy === -1
			? { variants: count }
			: { variants: count, coveredBy };
	});

const size = (set: Parameters): number => Object.keys(set).length;

/** The variants the box draws: those any row matches, or every one when it has no rows. */
export const matchedVariants = (
	sets: readonly Parameters[],
	variants: readonly Parameters[],
): number =>
	sets.length === 0
		? variants.length
		: variants.filter((variant) => sets.some((set) => matchesSet(variant, set)))
				.length;
