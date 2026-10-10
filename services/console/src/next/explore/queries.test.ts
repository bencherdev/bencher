import { expect, test } from "vitest";
import type { Api } from "../api";
import { type ExploreQuery, blankQuery } from "../query/query";
import { perfQuery } from "./queries";

const uuid = (n: number) =>
	`00000000-0000-4000-8000-${n.toString(16).padStart(12, "0")}`;

const QUERY: ExploreQuery = {
	...blankQuery(),
	branches: [{ uuid: uuid(1) }],
	testbeds: [{ uuid: uuid(3) }],
	benchmarks: [uuid(5)],
	measures: [uuid(7)],
};

const api = {} as Api;
const key = (query: ExploreQuery) =>
	perfQuery(api, "hashbrown", query).queryKey;

// Kills a view change that misses the answer the cache holds and asks again.
test("the plot query's answer is cached apart from the view", () => {
	expect(
		key({
			...QUERY,
			xAxis: "version",
			yScale: "log",
			layout: "stacked",
			hide: ["k1"],
			focus: "k2",
		}),
	).toEqual(key(QUERY));
	expect(key({ ...QUERY, benchmarks: [uuid(5), uuid(6)] })).not.toEqual(
		key(QUERY),
	);
});
