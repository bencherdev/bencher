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
