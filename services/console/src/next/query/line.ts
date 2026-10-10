import {
	type ExploreQuery,
	type Parameters,
	canonicalParameters,
} from "./query";

/** What makes a line one line: UUIDs, the variant's parameters, and the metric name. */
export interface LineIdentity {
	readonly branch: string;
	readonly testbed: string;
	readonly benchmark: string;
	readonly parameters: Parameters;
	readonly measure: string;
	readonly metric: string;
}

/** A short name for a line that stays the same as the query around it changes. */
export const lineKey = (line: LineIdentity): string =>
	hash53(
		JSON.stringify([
			line.branch,
			line.testbed,
			line.benchmark,
			canonicalParameters(line.parameters),
			line.measure,
			line.metric,
		]),
	).toString(36);

// cyrb53: 53 bits, so a key fits a JavaScript number and eleven base 36 digits.
const hash53 = (text: string): number => {
	let h1 = 0xdeadbeef;
	let h2 = 0x41c6ce57;
	for (let index = 0; index < text.length; index++) {
		const code = text.charCodeAt(index);
		h1 = Math.imul(h1 ^ code, 2654435761);
		h2 = Math.imul(h2 ^ code, 1597334677);
	}
	h1 = Math.imul(h1 ^ (h1 >>> 16), 2246822507);
	h1 ^= Math.imul(h2 ^ (h2 >>> 13), 3266489909);
	h2 = Math.imul(h2 ^ (h2 >>> 16), 2246822507);
	h2 ^= Math.imul(h1 ^ (h1 >>> 13), 3266489909);
	return 4294967296 * (2097151 & h2) + (h1 >>> 0);
};

export const lineVisible = (
	query: Pick<ExploreQuery, "hide" | "only">,
	key: string,
): boolean =>
	(query.only === undefined || query.only.includes(key)) &&
	!query.hide.includes(key);

/** Trade `only` for the hidden extras among the drawn lines, so lines a later edit adds are shown. */
export const settleVisibility = (
	query: ExploreQuery,
	keys: readonly string[],
): ExploreQuery => {
	const { only, ...rest } = query;
	if (only === undefined) {
		return query;
	}
	const hidden = keys.filter((key) => !lineVisible(query, key));
	return {
		...rest,
		hide: [...hidden, ...query.hide.filter((key) => !keys.includes(key))],
	};
};
