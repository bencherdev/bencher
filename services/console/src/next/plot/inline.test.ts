import { describe, expect, test } from "vitest";
import { INLINE_PAD, inlineGeometry, inlineOf } from "./inline";
import { DAY, RUNS, START } from "./testing";

const WIDTH = 150;
const HEIGHT = 30;
const LEFT = INLINE_PAD.left;
const RIGHT = WIDTH - INLINE_PAD.right;
const TOP = INLINE_PAD.top;
const BOTTOM = HEIGHT - INLINE_PAD.bottom;

describe("inlineGeometry", () => {
	// Kills an x that ignores time or a y axis drawn upside down.
	test("maps time across the width and higher values up", () => {
		const { points } = inlineGeometry(
			[0, 10, 40],
			{ y: [1, 3, 2], alerts: [] },
			WIDTH,
			HEIGHT,
		);
		const [first, second, third] = points;
		expect(first?.x).toBe(LEFT);
		expect(second?.x).toBeCloseTo(LEFT + (RIGHT - LEFT) / 4);
		expect(third?.x).toBe(RIGHT);
		expect(first?.y).toBeGreaterThan(third?.y ?? 0);
		expect(third?.y).toBeGreaterThan(second?.y ?? 0);
		for (const point of points) {
			expect(point?.y).toBeGreaterThan(TOP);
			expect(point?.y).toBeLessThan(BOTTOM);
		}
	});

	test("skips the x where the line has no point", () => {
		const { points } = inlineGeometry(
			[0, 1, 2],
			{ y: [1, null, 2], alerts: [] },
			WIDTH,
			HEIGHT,
		);
		expect(points[1]).toBeNull();
	});

	// Kills a scale that ignores the limits, which would push the band off the row.
	test("fits the limits inside the row with the values", () => {
		const { band } = inlineGeometry(
			[0, 1],
			{ y: [10, 10], upper: [100, 100], alerts: [] },
			WIDTH,
			HEIGHT,
		);
		expect(band.upper).toHaveLength(1);
		for (const { y } of band.upper[0] ?? []) {
			expect(y).toBeGreaterThan(TOP);
			expect(y).toBeLessThan(BOTTOM);
		}
	});

	// Kills an area that closes on the wrong edge for an upper limit.
	test("closes an upper limit's band on the bottom edge", () => {
		const { band } = inlineGeometry(
			[0, 1],
			{ y: [10, 10], upper: [12, 12], alerts: [] },
			WIDTH,
			HEIGHT,
		);
		expect(band.areas[0]?.map(({ bottom }) => bottom)).toEqual([
			BOTTOM,
			BOTTOM,
		]);
	});

	test("marks each alerting point where it is drawn", () => {
		const { points, alerts } = inlineGeometry(
			[0, 1, 2],
			{ y: [1, 2, 9], upper: [3, 3, 3], alerts: [2] },
			WIDTH,
			HEIGHT,
		);
		expect(alerts).toEqual([points[2]]);
	});

	test("centers a single point", () => {
		const { points } = inlineGeometry(
			[5],
			{ y: [1], alerts: [] },
			WIDTH,
			HEIGHT,
		);
		expect(points[0]?.x).toBe((LEFT + RIGHT) / 2);
	});
});

describe("inlineOf", () => {
	const [line] = RUNS.lines;
	const history = {
		y: line?.y ?? [],
		upper: line?.upper ?? [],
		alerts: line?.alerts ?? [],
	};

	// Kills a row's inline history that zigzags through every iteration, or redraws one run once.
	test("runs through the iteration each report shows", () => {
		expect(inlineOf(RUNS, history, undefined)).toEqual({
			x: RUNS.x,
			line: { ...history, y: [10, null, null, 12, null, 13, null] },
		});
		const once = { x: [1, 2], report: [0, 1] };
		const plain = { y: [1, 2], alerts: [] };
		expect(inlineOf(once, plain, undefined).line).toBe(plain);
	});

	// Kills a chosen iteration the inline history ignores, or whose alerts stay at their old index.
	test("draws only the chosen iteration", () => {
		expect(inlineOf(RUNS, history, 2)).toEqual({
			x: [START + DAY, START + 2 * DAY],
			line: { y: [14, 13], upper: [15, 12], alerts: [1] },
		});
	});
});
