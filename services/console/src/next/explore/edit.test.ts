import { describe, expect, test } from "vitest";
import { type ExploreQuery, blankQuery } from "../query/query";
import { withValue, withoutValue } from "./edit";

const uuid = (n: number) =>
	`00000000-0000-4000-8000-${n.toString(16).padStart(12, "0")}`;

const QUERY: ExploreQuery = {
	...blankQuery(),
	branches: [{ uuid: uuid(1), head: uuid(11) }],
	benchmarks: [uuid(5)],
	sets: [{ size: 64 }],
	metrics: ["value"],
};

describe("withValue", () => {
	// Kills adding a value twice, which the API would draw once and the box would show twice.
	test("add each value once, in the order picked", () => {
		const twice = withValue(
			withValue(QUERY, "benchmarks", uuid(6)),
			"benchmarks",
			uuid(6),
		);
		expect(twice.benchmarks).toEqual([uuid(5), uuid(6)]);
		expect(withValue(QUERY, "branches", uuid(1)).branches).toEqual(
			QUERY.branches,
		);
		expect(withValue(QUERY, "branches", uuid(2)).branches).toEqual([
			...QUERY.branches,
			{ uuid: uuid(2) },
		]);
		expect(withValue(QUERY, "metrics", "p99").metrics).toEqual([
			"value",
			"p99",
		]);
		expect(withValue(QUERY, "sets", { simd: "avx2" }).sets).toEqual([
			{ size: 64 },
			{ simd: "avx2" },
		]);
	});

	// Kills a ninth value, which the API would never read.
	test("stop at eight values", () => {
		let query = QUERY;
		for (let n = 10; n < 20; n++) {
			query = withValue(query, "measures", uuid(n));
		}
		expect(query.measures).toHaveLength(8);
	});
});

describe("withoutValue", () => {
	// Kills removing the wrong entry or every equal one.
	test("remove the entry at its position", () => {
		const query = { ...QUERY, sets: [{ size: 64 }, { size: 64 }, {}] };
		expect(withoutValue(query, "sets", 1).sets).toEqual([{ size: 64 }, {}]);
		expect(withoutValue(QUERY, "branches", 0).branches).toEqual([]);
	});
});
