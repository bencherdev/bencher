import type { JsonVariant } from "../../types/bencher";

/** A variant's parameters as `key=value` tags, in key order. */
export const tagsOf = (variant: JsonVariant) =>
	Object.keys(variant.parameters)
		.sort()
		.map((key) => `${key}=${variant.parameters[key]}`);

/**
 * Every benchmark is born with an empty variant, which a run that reports
 * parameters never uses; it shows only when it is the only one.
 */
export const shownVariants = (variants: JsonVariant[]) => {
	const tagged = variants.filter(
		(variant) => Object.keys(variant.parameters).length > 0,
	);
	return tagged.length > 0 ? tagged : variants;
};

/** Each parameter key in use, in order, with its values and how many variants carry each. */
export const parametersInUse = (variants: JsonVariant[]) => {
	const keys = new Map<string, Map<string, number>>();
	for (const variant of variants) {
		for (const [key, value] of Object.entries(variant.parameters)) {
			const values = keys.get(key) ?? new Map<string, number>();
			values.set(String(value), (values.get(String(value)) ?? 0) + 1);
			keys.set(key, values);
		}
	}
	return [...keys.keys()].sort().map((key) => ({
		key,
		values: [...(keys.get(key)?.entries() ?? [])]
			.sort(([a], [b]) => byValue(a, b))
			.map(([value, count]) => ({ value, variants: count })),
	}));
};

/** How many of a list's `total` variants it shows: all but the empty variant it hides. */
export const shownTotal = (variants: JsonVariant[], total: number) =>
	total - (variants.length - shownVariants(variants).length);

const byValue = (a: string, b: string) => {
	const x = Number(a);
	const y = Number(b);
	return Number.isNaN(x) || Number.isNaN(y) ? a.localeCompare(b) : x - y;
};
