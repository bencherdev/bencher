import { describe, expect, test } from "vitest";
import { type ExploreQuery, blankQuery } from "../query/query";
import { drawable, drawsPlot, perfSearch, requestKey } from "./request";

const uuid = (n: number) =>
	`00000000-0000-4000-8000-${n.toString(16).padStart(12, "0")}`;

const DAY = 24 * 60 * 60;
const NOW = 1_789_344_000_000;

const QUERY: ExploreQuery = {
	...blankQuery(),
	branches: [{ uuid: uuid(1) }, { uuid: uuid(2), head: uuid(12) }],
	testbeds: [{ uuid: uuid(3), spec: uuid(13) }, { uuid: uuid(4) }],
	benchmarks: [uuid(5), uuid(6)],
	measures: [uuid(7), uuid(8)],
};

/** The search as the API reads it: each list split on commas, then each entry decoded. */
const read = (search: string) => {
	const params = new URLSearchParams(search);
	const list = (name: string) =>
		params.get(name)?.split(",").map(decodeURIComponent);
	return { params, list };
};

describe("drawable", () => {
	// Kills asking the API without one of the four boxes it requires.
	test("needs a branch, a testbed, a benchmark, and a measure", () => {
		expect(drawable(QUERY)).toBe(true);
		for (const box of [
			"branches",
			"testbeds",
			"benchmarks",
			"measures",
		] as const) {
			expect(drawable({ ...QUERY, [box]: [] })).toBe(false);
		}
		expect(drawable({ ...QUERY, sets: [], metrics: [] })).toBe(true);
	});

	// Kills a pin's link that waits to ask for the plot's code until the pin
	// answers, and a query that cannot draw loading it.
	test("Explore draws a plot for a query that can draw, or for a pin", () => {
		expect(drawsPlot(QUERY)).toBe(true);
		expect(drawsPlot({ ...blankQuery(), plot: uuid(9) })).toBe(true);
		expect(drawsPlot({ ...QUERY, branches: [] })).toBe(false);
		expect(drawsPlot(blankQuery())).toBe(false);
	});
});

describe("perfSearch", () => {
	// Kills a head or a spec read against the wrong entry, or a box left out.
	test("name every box, with heads and specs beside their entries", () => {
		const { params } = read(perfSearch(QUERY, NOW));
		expect(params.get("branches")).toBe(`${uuid(1)},${uuid(2)}`);
		expect(params.get("heads")).toBe(`,${uuid(12)}`);
		expect(params.get("testbeds")).toBe(`${uuid(3)},${uuid(4)}`);
		expect(params.get("specs")).toBe(`${uuid(13)},`);
		expect(params.get("benchmarks")).toBe(`${uuid(5)},${uuid(6)}`);
		expect(params.get("measures")).toBe(`${uuid(7)},${uuid(8)}`);
	});

	// Kills sending empty heads or specs, and an empty filter the API would refuse.
	test("leave out what the query does not narrow", () => {
		const { params } = read(
			perfSearch(
				{
					...QUERY,
					branches: [{ uuid: uuid(1) }],
					testbeds: [{ uuid: uuid(3) }],
				},
				NOW,
			),
		);
		for (const name of ["heads", "specs", "parameters", "metrics"]) {
			expect(params.has(name)).toBe(false);
		}
	});

	// Kills a set or a name whose own commas split it on the API's side.
	test("encode each set and metric name inside its list", () => {
		const sets = [
			{ input_bytes: 65536, simd: "avx2" },
			{ label: "a,b&c=d %2C" },
		];
		const metrics = ["value", "p99, tail"];
		const { list } = read(perfSearch({ ...QUERY, sets, metrics }, NOW));
		expect(list("parameters")?.map((set) => JSON.parse(set))).toEqual(sets);
		expect(list("metrics")).toEqual(metrics);
	});

	// Kills a rolling window that ignores its end or measures in the wrong unit.
	test("end a rolling window now, or at its own end", () => {
		const rolling = (window: ExploreQuery["window"]) => {
			const { params } = read(perfSearch({ ...QUERY, window }, NOW));
			return [Number(params.get("start_time")), Number(params.get("end_time"))];
		};
		expect(rolling({ seconds: 7 * DAY })).toEqual([NOW - 7 * DAY * 1000, NOW]);
		expect(rolling({ seconds: DAY, end: 1_000_000_000_000 })).toEqual([
			1_000_000_000_000 - DAY * 1000,
			1_000_000_000_000,
		]);
		expect(rolling({ start: 5, end: 9 })).toEqual([5, 9]);
		expect(rolling({ start: 5 })).toEqual([5, NOW]);
	});
});

describe("requestKey", () => {
	// Kills a refetch when only the view changes: a key toggle must not ask the API.
	test("stay the same when only the view changes", () => {
		const key = requestKey(QUERY);
		for (const view of [
			{ hide: ["a1"] },
			{ only: ["a1"] },
			{ focus: "a1" },
			{ layout: "stacked" },
			{ xAxis: "version" },
			{ yScale: "log" },
			{ report: uuid(9) },
			{ plot: uuid(10) },
		] as const) {
			expect(requestKey({ ...QUERY, ...view })).toBe(key);
		}
	});

	// Kills a key that misses a box or the window, which would draw stale lines.
	test("change with every box and the window", () => {
		const key = requestKey(QUERY);
		for (const change of [
			{ branches: [{ uuid: uuid(1) }] },
			{ branches: [{ uuid: uuid(1) }, { uuid: uuid(2) }] },
			{ testbeds: [{ uuid: uuid(3) }, { uuid: uuid(4) }] },
			{ benchmarks: [uuid(5)] },
			{ sets: [{ simd: "avx2" }] },
			{ measures: [uuid(7)] },
			{ metrics: ["value"] },
			{ window: { seconds: 7 * DAY } },
		]) {
			expect(requestKey({ ...QUERY, ...change })).not.toBe(key);
		}
	});
});
