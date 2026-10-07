import { describe, expect, test } from "vitest";
import { bandGeometry, EDGE } from "./band";

describe("bandGeometry", () => {
	// Kills shading toward the wrong side: an upper limit guards above, so the band runs down to the bottom edge.
	test("shades from an upper limit down to the bottom edge", () => {
		expect(bandGeometry([1, 2, 3], undefined, [5, 6, 7])).toEqual({
			areas: [
				[
					{ index: 0, top: 5, bottom: EDGE },
					{ index: 1, top: 6, bottom: EDGE },
					{ index: 2, top: 7, bottom: EDGE },
				],
			],
			lower: [],
			upper: [[0, 1, 2]],
		});
	});

	test("shades from a lower limit up to the top edge", () => {
		expect(bandGeometry([5, 6], [1, 2], undefined)).toEqual({
			areas: [
				[
					{ index: 0, top: EDGE, bottom: 1 },
					{ index: 1, top: EDGE, bottom: 2 },
				],
			],
			lower: [[0, 1]],
			upper: [],
		});
	});

	test("shades between a lower and an upper limit", () => {
		expect(bandGeometry([5], [1], [9]).areas).toEqual([
			[{ index: 0, top: 9, bottom: 1 }],
		]);
	});

	// Kills drawing a band across a point that no threshold checked.
	test("breaks at a point of the line that has no limit", () => {
		expect(
			bandGeometry([1, 1, 1, 1, 1], undefined, [5, 5, null, 5, 5]),
		).toEqual({
			areas: [
				[
					{ index: 0, top: 5, bottom: EDGE },
					{ index: 1, top: 5, bottom: EDGE },
				],
				[
					{ index: 3, top: 5, bottom: EDGE },
					{ index: 4, top: 5, bottom: EDGE },
				],
			],
			lower: [],
			upper: [
				[0, 1],
				[3, 4],
			],
		});
	});

	// Kills breaking the band where another line's report sits and this line has no point.
	test("runs across x where the line has no point", () => {
		expect(
			bandGeometry([1, null, 1, null, 1], undefined, [5, null, 6, null, 7])
				.upper,
		).toEqual([[0, 2, 4]]);
	});

	test("keeps each side's limit runs apart when only one side breaks", () => {
		const { lower, upper } = bandGeometry([5, 5, 5], [1, null, 1], [9, 9, 9]);
		expect(lower).toEqual([[0], [2]]);
		expect(upper).toEqual([[0, 1, 2]]);
	});

	test("draws nothing for a line no threshold checks", () => {
		expect(bandGeometry([1, 2], undefined, undefined)).toEqual({
			areas: [],
			lower: [],
			upper: [],
		});
	});
});
