import { describe, expect, test } from "vitest";
import type { JsonConsolePerfLine } from "../../types/bencher";
import { DEFAULT_WINDOW, type ExploreQuery, blankQuery } from "../query/query";
import {
	clearable,
	hasValues,
	metricNames,
	settleFocus,
	withValue,
	withoutValue,
} from "./edit";

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

// Kills a Clear hidden while only the x axis, the y scale, the window, the
// layout, or the iteration is off its default, or one drawn for a blank query,
// a rolling window that ends now, or one that only names the default length.
test("clearable is true for a value or a view control off its default", () => {
	expect(clearable(blankQuery())).toBe(false);
	expect(
		clearable({ ...blankQuery(), window: { seconds: DEFAULT_WINDOW } }),
	).toBe(false);
	for (const view of [
		{ xAxis: "version" },
		{ yScale: "linear" },
		{ window: { seconds: 7 * 24 * 60 * 60 } },
		{ window: { seconds: DEFAULT_WINDOW, end: 1_787_011_199_999 } },
		{ window: { start: 1_786_924_800_000 } },
		{ layout: "stacked" },
		{ iteration: 2 },
		{ measures: [uuid(3)] },
	] as const) {
		expect(clearable({ ...blankQuery(), ...view })).toBe(true);
	}
});

describe("settleFocus", () => {
	const focused = { ...QUERY, focus: "1gav3bmyc9r" };

	// Kills keeping the focus on a line a box edit may no longer draw, or a box left out of the comparison.
	test("drop the focus when any box changes", () => {
		for (const change of [
			{ branches: [{ uuid: uuid(1), head: uuid(12) }] },
			{ testbeds: [{ uuid: uuid(2), spec: uuid(13) }] },
			{ benchmarks: [] },
			{ sets: [{ size: 128 }] },
			{ measures: [uuid(3)] },
			{ metrics: [] },
		] as const) {
			expect(settleFocus(focused, { ...focused, ...change }).focus).toBe(
				undefined,
			);
		}
	});

	// Kills dropping the focus on an edit that draws the same lines.
	test("keep the focus through a view change", () => {
		for (const change of [
			{ yScale: "log" },
			{ window: { seconds: DEFAULT_WINDOW / 4 } },
			{ hide: ["z"] },
		] as const) {
			expect(settleFocus(focused, { ...focused, ...change }).focus).toBe(
				focused.focus,
			);
		}
	});

	// Kills comparing the boxes by reference, which drops the focus on every edit.
	test("keep the focus when the boxes hold the same values", () => {
		const current = { ...focused, sets: [{ simd: "avx2", size: 64 }] };
		const copy = {
			...structuredClone(current),
			sets: [{ size: 64, simd: "avx2" }],
		};
		expect(settleFocus(current, copy).focus).toBe(current.focus);
	});
});

describe("metricNames", () => {
	// Kills repeating a name, a name ranked by its first line alone or in
	// drawing order, a thinned line's point read at its value's position, and
	// a name with no point in the window ahead of one with a point.
	test("list each drawn metric name once, the one with the newest point first", () => {
		const line = (
			metric: string,
			y: (number | null)[],
			index?: number[],
		): JsonConsolePerfLine => ({
			branch: 0,
			testbed: 0,
			benchmark: 0,
			variant: 0,
			measure: 0,
			metric,
			series: { y, ...(index && { index }), alerts: [] },
		});
		const lines = [
			line("p50", [null, null, null]),
			line("value", [1, null, null]),
			line("max", [8, null, null]),
			line("p99", [5], [1]),
			line("value", [null, null, 3]),
		];
		expect(
			metricNames({
				points: { x: [1000, 2000, 3000], report: [0, 1, 2] },
				lines,
			}),
		).toEqual(["value", "p99", "max", "p50"]);
	});
});
