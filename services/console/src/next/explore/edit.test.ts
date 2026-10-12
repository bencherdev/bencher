import { describe, expect, test } from "vitest";
import { type ExploreQuery, blankQuery } from "../query/query";
import { hasValues, withValue, withoutValue, withoutValues } from "./edit";

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

// Kills a Clear that leaves a box, a hidden or focused line, the report, or
// the pin behind, or resets the window, the axes, or the layout.
test("withoutValues empties every box and keeps the view", () => {
	const view = {
		xAxis: "version",
		yScale: "log",
		window: { start: 1_786_924_800_000, end: 1_787_011_199_999 },
		layout: "stacked",
	} as const;
	const full: ExploreQuery = {
		...QUERY,
		...view,
		testbeds: [{ uuid: uuid(2), spec: uuid(12) }],
		measures: [uuid(3)],
		hide: ["a1"],
		only: ["b2"],
		focus: "a1",
		report: uuid(7),
		plot: uuid(8),
	};
	expect(withoutValues(full)).toEqual({ ...blankQuery(), ...view });
});

// Kills a Clear drawn only for some boxes, such as one that needs a benchmark,
// or for the view alone.
test("hasValues is true for a value in any one box and false for the view alone", () => {
	const view: ExploreQuery = {
		...blankQuery(),
		yScale: "log",
		window: { seconds: 7 * 24 * 60 * 60 },
	};
	expect(hasValues(view)).toBe(false);
	for (const one of [
		{ branches: [{ uuid: uuid(1) }] },
		{ testbeds: [{ uuid: uuid(2) }] },
		{ benchmarks: [uuid(5)] },
		{ sets: [{}] },
		{ measures: [uuid(3)] },
		{ metrics: ["value"] },
	]) {
		expect(hasValues({ ...view, ...one })).toBe(true);
	}
});
