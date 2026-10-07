import { describe, expect, test } from "vitest";
import { alignToAxis, extent, sameX } from "./columns";
import { testLine } from "./testing";
import type { PlotData } from "./types";

// Two branches whose version numbers interleave out of time order.
const TWO_BRANCHES: PlotData = {
	x: [1000, 2000, 3000, 4000],
	report: [0, 1, 2, 3],
	reports: [
		{ uuid: "r0", version: 7 },
		{ uuid: "r1", version: 3 },
		{ uuid: "r2", version: 8 },
		{ uuid: "r3", version: 4 },
	],
	measures: [{ name: "Latency", units: "nanoseconds (ns)" }],
	lines: [
		testLine({
			id: "main",
			y: [10, null, 11, null],
			upper: [12, null, 13, null],
			alerts: [2],
		}),
		testLine({
			id: "feature",
			branch: "feature",
			y: [null, 20, null, 21],
			baseline: [null, 19, null, 20],
			alerts: [1],
		}),
	],
};

describe("alignToAxis", () => {
	test("keeps a date axis as it came", () => {
		expect(alignToAxis(TWO_BRANCHES, "date")).toBe(TWO_BRANCHES);
	});

	// Kills drawing a version axis in time order, which uPlot cannot take.
	test("sorts every column by version number on a version axis", () => {
		const aligned = alignToAxis(TWO_BRANCHES, "version");
		expect(aligned.x).toEqual([3, 4, 7, 8]);
		expect(aligned.report).toEqual([1, 3, 0, 2]);
		expect(aligned.lines[0]?.y).toEqual([null, null, 10, 11]);
		expect(aligned.lines[0]?.upper).toEqual([null, null, 12, 13]);
		expect(aligned.lines[1]?.baseline).toEqual([19, 20, null, null]);
	});

	// Kills alert indices left pointing at the time order.
	test("moves alert indices with their points", () => {
		const aligned = alignToAxis(TWO_BRANCHES, "version");
		expect(aligned.lines[0]?.alerts).toEqual([3]);
		expect(aligned.lines[1]?.alerts).toEqual([0]);
	});

	test("keeps time order between reports of one version", () => {
		const aligned = alignToAxis(
			{
				...TWO_BRANCHES,
				reports: [
					{ uuid: "r0", version: 5 },
					{ uuid: "r1", version: 2 },
					{ uuid: "r2", version: 5 },
					{ uuid: "r3", version: 2 },
				],
			},
			"version",
		);
		expect(aligned.report).toEqual([1, 3, 0, 2]);
	});
});

describe("sameX", () => {
	// Points of one report's iterations, or of one version, share an x.
	test("spans every point that shares the index's x", () => {
		const x = [1, 2, 2, 2, 3];
		expect(sameX(x, 2)).toEqual([1, 4]);
		expect(sameX(x, 0)).toEqual([0, 1]);
		expect(sameX(x, 4)).toEqual([4, 5]);
	});
});

describe("extent", () => {
	// Kills a range that leaves out the limits, which would push a band off the plot.
	test("spans the values and both limits, skipping gaps", () => {
		expect(
			extent([
				{ y: [5, null, 7], lower: [4, null, null], upper: [null, null, 9] },
				{ y: [6] },
			]),
		).toEqual([4, 9]);
	});

	test("has none without a value", () => {
		expect(extent([{ y: [null] }])).toBeUndefined();
	});
});
