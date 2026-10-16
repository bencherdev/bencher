import { describe, expect, test } from "vitest";
import { clicked, readout, readoutLeft } from "./readout";
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

	// Kills an alert read off another line or point, or named on a row the reader is not focused on.
	test("names the alert raised at the focused line's point", () => {
		const raised = { index: 1, uuid: "alert", status: "active" } as const;
		const data = {
			...DATA,
			lines: DATA.lines.map((line, index) =>
				index === 0
					? { ...line, alerts: [1], raised: [raised] }
					: { ...line, alerts: [1], raised: [{ ...raised, uuid: "other" }] },
			),
		};
		expect(
			readout(data, 1, [0, 2], 0, SCALES, "date", "UTC").rows.map(
				({ alert }) => alert,
			),
		).toEqual([raised, undefined]);
		expect(
			readout(data, 0, [0, 2], 0, SCALES, "date", "UTC").rows[0]?.alert,
		).toBeUndefined();
		expect(
			readout(data, 1, [0, 2], 2, SCALES, "date", "UTC").rows.map(
				({ alert }) => alert,
			),
		).toEqual([{ ...raised, uuid: "other" }, undefined]);
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

describe("clicked", () => {
	// Five reports, the middle three at one x, as on a version axis.
	const XS = [10, 20, 20, 20, 30];
	const point = (index: number, line: number | null = 0, panel = 0) => ({
		index,
		panel,
		x: (XS[index] ?? 0) * 10,
		line,
	});
	const pinned = { ...point(1), pinned: true };

	// Kills a click that only pins where nothing is pinned, or pins the wrong point.
	test("pins the point clicked when nothing is pinned or another point is", () => {
		expect(clicked(null, point(1), XS, false)).toEqual(pinned);
		expect(
			clicked({ ...point(1), pinned: false }, point(1), XS, false),
		).toEqual(pinned);
		expect(clicked(pinned, point(4), XS, false)).toEqual({
			...point(4),
			pinned: true,
		});
		expect(clicked(pinned, point(1, 0, 1), XS, false)).toEqual({
			...point(1, 0, 1),
			pinned: true,
		});
	});

	// Kills a click on the pinned point that pins it again, one that closes a mouse's readout instead of letting it follow, and one judged by the index where several reports share the x.
	test("unpins the pinned point for a mouse, at any index of its x, on its line or none", () => {
		expect(clicked(pinned, point(1), XS, false)).toEqual({
			...point(1),
			pinned: false,
		});
		expect(clicked(pinned, point(3), XS, false)).toEqual({
			...point(3),
			pinned: false,
		});
		expect(clicked(pinned, point(2, null), XS, false)).toEqual({
			...point(2, null),
			pinned: false,
		});
	});

	// Kills a tap on the pinned point that leaves a readout no finger can move.
	test("closes the pinned point for a tap", () => {
		expect(clicked(pinned, point(2), XS, true)).toBeNull();
	});

	// Kills a click on another line's point at the same x that unpins instead of moving the pin to it.
	test("moves the pin to another line's point at the same x", () => {
		expect(clicked(pinned, point(1, 1), XS, false)).toEqual({
			...point(1, 1),
			pinned: true,
		});
	});

	// Kills a pin from the keys, on no line, that the first click on any line's point at its x moves instead of unpinning.
	test("unpins a pin on no line from a click on any line at its x", () => {
		const keyed = { ...point(1, null), pinned: true };
		expect(clicked(keyed, point(1, 1), XS, false)).toEqual({
			...point(1, 1),
			pinned: false,
		});
	});
});
