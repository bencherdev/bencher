import type { JsonConsolePerf } from "../../types/bencher";
import type { ExploreQuery } from "../query/query";

type Dimension =
	| "branches"
	| "testbeds"
	| "benchmarks"
	| "sets"
	| "measures"
	| "metrics";

/** What a narrow chip says of its box: the values by name once the plot has named them all. */
export const chipSummary = (
	query: ExploreQuery,
	dimension: Dimension,
	perf: JsonConsolePerf | undefined,
	blank: boolean,
): string => {
	if (dimension === "sets") {
		const [only, ...rest] = query.sets;
		if (only === undefined) {
			return "every variant";
		}
		if (rest.length > 0) {
			return `${query.sets.length} sets`;
		}
		const tags = Object.entries(only).map(
			([name, value]) => `${name}=${String(value)}`,
		);
		return tags.join(" ") || "every variant";
	}
	if (dimension === "metrics") {
		return query.metrics.join(", ") || "every metric";
	}
	const uuids =
		dimension === "branches" || dimension === "testbeds"
			? query[dimension].map(({ uuid }) => uuid)
			: query[dimension];
	if (uuids.length === 0) {
		return blank && dimension === "benchmarks" ? "add one" : "none";
	}
	const table: { uuid: string; name: string }[] = perf?.[dimension] ?? [];
	const names = uuids.map(
		(uuid) => table.find((entry) => entry.uuid === uuid)?.name,
	);
	return names.every((name) => name !== undefined)
		? names.join(", ")
		: `${uuids.length} chosen`;
};
