import { describe, expect, test } from "vitest";
import { exploreSearchFromClassic } from "./classic";
import { decodeQuery } from "./query";

const uuid = (n: number) =>
	`00000000-0000-4000-8000-${n.toString(16).padStart(12, "0")}`;

// A classic perf link as the classic console writes it, lists joined by an encoded comma.
const CLASSIC = new URLSearchParams({
	report: uuid(9),
	branches: `${uuid(1)},${uuid(2)}`,
	heads: `${uuid(11)},`,
	testbeds: uuid(3),
	specs: uuid(13),
	benchmarks: `${uuid(5)},${uuid(6)}`,
	measures: uuid(7),
	start_time: "1757000000000",
	end_time: "1757800000000",
	tab: "benchmarks",
	key: "true",
	x_axis: "version",
	y_axis: "log",
	lower_value: "false",
	upper_value: "false",
	lower_boundary: "true",
	upper_boundary: "false",
	clear: "true",
	reports_per_page: "4",
	reports_page: "1",
	branches_search: "main",
	plot: uuid(10),
	embed_title: "Hashes",
}).toString();

describe("exploreSearchFromClassic", () => {
	// Kills misreading any name the classic perf page shares with Explore.
	test("keep the query of a classic perf link", () => {
		expect(decodeQuery(exploreSearchFromClassic(CLASSIC))).toEqual({
			branches: [{ uuid: uuid(1), head: uuid(11) }, { uuid: uuid(2) }],
			testbeds: [{ uuid: uuid(3), spec: uuid(13) }],
			benchmarks: [uuid(5), uuid(6)],
			sets: [],
			measures: [uuid(7)],
			metrics: [],
			xAxis: "version",
			yScale: "log",
			window: { start: 1_757_000_000_000, end: 1_757_800_000_000 },
			layout: "dual",
			hide: [],
			report: uuid(9),
			plot: uuid(10),
		});
	});

	// Kills carrying the classic page's own paging, tabs, and toggles into Explore's link.
	test("drop what only the classic page reads", () => {
		const names = new Set(
			new URLSearchParams(exploreSearchFromClassic(CLASSIC)).keys(),
		);
		for (const name of [
			"tab",
			"key",
			"lower_value",
			"upper_value",
			"lower_boundary",
			"upper_boundary",
			"clear",
			"reports_per_page",
			"reports_page",
			"branches_search",
			"embed_title",
		]) {
			expect(names.has(name)).toBe(false);
		}
	});

	// Kills ignoring the old name for the x axis, or letting it override the new one.
	test("read the old range name for the x axis", () => {
		const read = (search: string) =>
			decodeQuery(exploreSearchFromClassic(search)).xAxis;
		expect(read("range=version")).toBe("version");
		expect(read("x_axis=date_time&range=version")).toBe("date");
		expect(read("x_axis=nope&range=version")).toBe("version");
	});
});

describe("a classic bound toggle", () => {
	const metrics = (search: string) =>
		decodeQuery(exploreSearchFromClassic(search)).metrics;

	// Kills a bound the classic link turned on dropped, a bound it left off
	// drawn, and the value itself left out beside a bound.
	test("draws the value and each bound it turned on", () => {
		expect(metrics("lower_value=true")).toEqual(["value", "lower_value"]);
		expect(metrics("upper_value=true&lower_value=false")).toEqual([
			"value",
			"upper_value",
		]);
		expect(metrics("lower_value=true&upper_value=true")).toEqual([
			"value",
			"lower_value",
			"upper_value",
		]);
	});

	// Kills a classic link with no bound turned on narrowed to the value, which
	// would draw nothing for a version 1 project's metrics.
	test("with none turned on draws every metric", () => {
		expect(metrics(CLASSIC)).toEqual([]);
		expect(metrics("lower_boundary=true&upper_boundary=true")).toEqual([]);
	});

	// Kills a toggle overriding the metrics a link names.
	test("leaves the metrics a link names alone", () => {
		expect(metrics("metrics=latency&lower_value=true")).toEqual(["latency"]);
	});

	// Kills a public plot or embed that reads the toggle only after a redirect.
	test("reads the same in place", () => {
		expect(decodeQuery("?upper_value=true").metrics).toEqual([
			"value",
			"upper_value",
		]);
	});
});
