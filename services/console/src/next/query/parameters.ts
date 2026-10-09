import type { Parameters } from "./query";

/** The set's JSON with its keys in RFC 8785 order, as the API canonicalizes it. */
export const canonicalParameters = (parameters: Parameters): string =>
	`{${Object.keys(parameters)
		// The default sort compares UTF-16 code units, which is RFC 8785's order.
		.sort()
		.map((key) => `${JSON.stringify(key)}:${JSON.stringify(parameters[key])}`)
		.join(",")}}`;
