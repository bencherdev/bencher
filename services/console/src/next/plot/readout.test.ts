import { describe, expect, test } from "vitest";
import { readout, readoutLeft } from "./readout";
import { START, testData, testLine } from "./testing";
import type { PlotData } from "./types";

const SCALES = [
	{ factor: 1e3, symbol: "µs" },
	{ factor: 1, symbol: "ops" },
];

const DATA: PlotData = {
	...testData([
		testLine({
			id: "a",
			y: [18_000, 20_600],
			baseline: [18_000, 18_100],
			upper: [19_000, 19_400],
		}),
		testLine({ id: "b", y: [17_000, null] }),
		testLine({ id: "c", measure: 1, y: [2.5, 3.25] }),
	]),
	measures: [
		{ name: "Latency", units: "nanoseconds (ns)" },
		{ name: "Throughput", units: "operations (ops)" },
	],
};

describe("readout", () => {
	// Kills formatting in raw units, reading the wrong index, or another measure's units.
	test("lists each visible line's value at the x in its measure's units", () => {
		const result = readout(DATA, 0, [0, 1, 2], null, SCALES, "date", "UTC");
		expect(result.rows.map(({ line, value }) => [line, value])).toEqual([
			[0, "18.00 µs"],
			[1, "17.00 µs"],
			[2, "2.50 ops"],
		]);
		expect(result.when).toBe("Sep 1, 12:00");
		expect(result.report).toBe(0);
	});

	test("leaves out a line with no point at the x", () => {
		const result = readout(DATA, 1, [0, 1, 2], null, SCALES, "date", "UTC");
		expect(result.rows.map(({ line }) => line)).toEqual([0, 2]);
	});

	// Kills listing hidden lines: the caller passes only the visible ones.
	test("lists only the lines it is given", () => {
		const result = readout(DATA, 0, [2], null, SCALES, "date", "UTC");
		expect(result.rows.map(({ line }) => line)).toEqual([2]);
	});

	// Kills a focused line that loses its place, its delta, or its limit.
	test("leads with the focused line, its delta, and its limit", () => {
		const result = readout(DATA, 1, [2, 0], 0, SCALES, "date", "UTC");
		expect(result.rows[0]).toEqual({
			line: 0,
			value: "20.60 µs",
			report: 1,
			focused: true,
			delta: { text: "+13.8%", tone: "worse" },
			limit: "limit 19.40 µs",
		});
		expect(result.rows[1]?.focused).toBe(false);
		expect(result.rows[1]?.delta).toBeUndefined();
	});

	test("says when no threshold checks the focused line", () => {
		const result = readout(DATA, 0, [1], 1, SCALES, "date", "UTC");
		expect(result.rows[0]?.limit).toBe("no threshold");
		expect(result.rows[0]?.delta).toBeNull();
	});

	// Kills an unbounded readout over dozens of lines.
	test("shows at most six lines and counts the rest", () => {
		const many = testData(
			Array.from({ length: 9 }, (_, index) =>
				testLine({ id: `l${index}`, y: [index + 1] }),
			),
		);
		const result = readout(
			many,
			0,
			[0, 1, 2, 3, 4, 5, 6, 7, 8],
			8,
			SCALES,
			"date",
			"UTC",
		);
		expect(result.rows.map(({ line }) => line)).toEqual([8, 0, 1, 2, 3, 4]);
		expect(result.more).toBe(3);
	});

	// Iterations of one report, or reports of one version, share an x.
	test("reads each line's last point among those that share the x", () => {
		const shared: PlotData = {
			x: [START, START, START + 1],
			report: [0, 1, 2],
			reports: [
				{ uuid: "r0", version: 1 },
				{ uuid: "r1", version: 1 },
				{ uuid: "r2", version: 2 },
			],
			measures: DATA.measures,
			lines: [
				testLine({ id: "a", y: [1000, null, 3000] }),
				testLine({ id: "b", y: [null, 2000, null] }),
			],
		};
		const result = readout(shared, 0, [0, 1], null, SCALES, "date", "UTC");
		expect(result.rows.map(({ line, report }) => [line, report])).toEqual([
			[0, 0],
			[1, 1],
		]);
	});

	test("names the version on a version axis", () => {
		const result = readout(DATA, 1, [0], null, SCALES, "version", "UTC");
		expect(result.when).toBe("Version 101");
	});
});

describe("readoutLeft", () => {
	// Kills a side picked by the cursor's half of the plot, which runs a wide readout off a narrow one.
	test("sits right of the cursor when it fits", () => {
		expect(readoutLeft(100, 200, 400)).toBe(112);
	});

	test("sits left of the cursor when only that side fits", () => {
		expect(readoutLeft(300, 200, 400)).toBe(88);
	});

	// Kills a readout pushed past either edge when neither side has room.
	test("stays inside the room when neither side fits", () => {
		expect(readoutLeft(150, 250, 320)).toBe(25);
		expect(readoutLeft(20, 300, 320)).toBe(4);
		expect(readoutLeft(300, 300, 320)).toBe(16);
	});
});
