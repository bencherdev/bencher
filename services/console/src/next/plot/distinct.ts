import { leafOf } from "../leaf";
import type { PlotParameterValue } from "./types";

// Names told apart within their list, in a module of their own so a page that names lines loads no plot key.

const valueKey = (value: PlotParameterValue | undefined): string =>
	value === undefined ? "" : JSON.stringify(value);

interface Parameterized {
	parameters: Readonly<Record<string, PlotParameterValue>>;
}

const keysOf = (lines: readonly Parameterized[]): string[] => {
	const keys = new Set<string>();
	for (const line of lines) {
		for (const key of Object.keys(line.parameters)) {
			keys.add(key);
		}
	}
	return [...keys];
};

/** The parameter keys whose values differ across the lines, a missing key counting as a value. */
export const varyingKeys = (lines: readonly Parameterized[]): string[] =>
	keysOf(lines).filter(
		(key) =>
			new Set(lines.map((line) => valueKey(line.parameters[key]))).size > 1,
	);

/**
 * The leading segments every name shares, cut after a `::` or `/`, and what
 * each name adds to them. Names that are all one name share it whole; a lone
 * name, or names with no segment in common, share nothing.
 */
export const distinct = (
	names: readonly string[],
): { prefix: string; rests: string[] } => {
	const [first = ""] = names;
	if (names.length < 2) {
		return { prefix: "", rests: [...names] };
	}
	if (names.every((name) => name === first)) {
		return { prefix: first, rests: names.map(() => "") };
	}
	let common = first.length;
	for (const name of names) {
		let at = 0;
		while (at < common && name[at] === first[at]) {
			at++;
		}
		common = at;
	}
	let cut = 0;
	for (const { index, 0: separator } of first
		.slice(0, common)
		.matchAll(/::|\//g)) {
		const end = index + separator.length;
		if (names.every((name) => name.length > end)) {
			cut = end;
		}
	}
	return {
		prefix: first.slice(0, cut),
		rests: names.map((name) => name.slice(cut)),
	};
};

/** Each name as a control among them calls it: what it adds to the prefix they share, or its last segment when it is alone or they share it whole. */
export const restsOf = (names: readonly string[]): string[] => {
	const { rests } = distinct(names);
	return names.map(
		(name, index) => (names.length > 1 && rests[index]) || leafOf(name),
	);
};
