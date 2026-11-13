import { describe, expect, test } from "vitest";
import { holdsTwo, pinched, scaled, within } from "./zoom";

const X = [10, 20, 20, 30, 40];

describe("within", () => {
	// Kills a span that drops a point on either end of the range, or keeps one past it.
	test("spans the indices whose x is inside the range, both ends included", () => {
		expect(within(X, [20, 30])).toEqual([1, 4]);
		expect(within(X, [15, 35])).toEqual([1, 4]);
		expect(within(X, [41, 50])).toEqual([5, 5]);
		expect(within(X, null)).toEqual([0, 5]);
	});
});

describe("holdsTwo", () => {
	// Kills a range that counts a single report's iterations as 2 points, a
	// hole as a point, or points of 2 lines as 2 of one.
	test("needs one column with points at 2 x inside the range", () => {
		expect(holdsTwo(X, [[1, 2, 3, 4, 5]], [20, 30])).toBe(true);
		expect(holdsTwo(X, [[1, 2, 3, 4, 5]], [15, 25])).toBe(false);
		expect(holdsTwo(X, [[1, 2, 3, null, 5]], [20, 30])).toBe(false);
		expect(
			holdsTwo(
				X,
				[
					[1, null, null, null, 5],
					[null, 2, null, null, null],
				],
				[10, 20],
			),
		).toBe(false);
	});
});

describe("scaled", () => {
	// Kills a scale that moves the value under the fingers, or scales about the range's middle.
	test("keeps the value at `at` where it was across the range", () => {
		expect(scaled([0, 100], 0.25, 0.5)).toEqual([12.5, 62.5]);
		expect(scaled([0, 100], 0.5, 2)).toEqual([-50, 150]);
	});
});

describe("pinched", () => {
	const x = [0, 10, 20, 30, 40, 50, 60, 70, 80, 90];
	const line = [[1, 2, 3, 4, 5, 6, 7, 8, 9, 10]];

	// Kills a pinch in that zooms past 2 points, and one out past every point.
	test("stays between the narrowest range that holds 2 points and every point", () => {
		expect(pinched(x, line, [0, 90], 0.5, 0.5, [0, 90])).toEqual([22.5, 67.5]);
		const [min, max] = pinched(x, line, [0, 90], 0.5, 0.01, [0, 90]) ?? [];
		expect(min).toBeLessThanOrEqual(40);
		expect(max).toBeGreaterThanOrEqual(50);
		expect((max ?? 0) - (min ?? 0)).toBeLessThan(10.01);
		expect(pinched(x, line, [20, 60], 0.5, 4, [0, 90])).toEqual([0, 90]);
	});

	// Kills a pinch in where no range holds 2 points.
	test("is none when the range it starts from holds fewer than 2 points", () => {
		expect(pinched(x, line, [41, 49], 0.5, 0.5, [0, 90])).toBeNull();
	});

	// Kills a pinch in past 2 points that stops short of them when there is room around them.
	test("narrows to 2 points with room around them", () => {
		const [min = 0, max = 0] =
			pinched(x, line, [35, 55], 0.5, 0.1, [0, 90]) ?? [];
		expect(min).toBeLessThanOrEqual(40);
		expect(min).toBeGreaterThan(39.99);
		expect(max).toBeGreaterThanOrEqual(50);
		expect(max).toBeLessThan(50.01);
	});

	// Kills a pinch in past 2 points that narrows by a sliver a range already at its narrowest.
	test("stays exactly where it started when only a sliver narrower holds 2 points", () => {
		expect(pinched(x, line, [40, 50], 0.3, 0.5, [0, 90])).toEqual([40, 50]);
		const narrowest = pinched(x, line, [35, 55], 0.5, 0.1, [0, 90]) ?? [0, 0];
		expect(pinched(x, line, narrowest, 0.5, 0.1, [0, 90])).toEqual(narrowest);
		// A billionth wider than its 2 points, and a pinch fine enough for the search to find that billionth.
		const wider = [40 - 5e-9, 50 + 5e-9] as const;
		expect(pinched(x, line, wider, 0.5, 0.999, [0, 90])).toEqual(wider);
	});
});
