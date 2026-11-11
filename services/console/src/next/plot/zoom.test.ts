import { describe, expect, test } from "vitest";
import { holdsTwo, within, zoomSpan } from "./zoom";

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

describe("zoomSpan", () => {
	// Kills versions read out at the drag's own ends rather than the whole versions inside it.
	test("names the versions inside a span on the version axis", () => {
		expect(zoomSpan([101.4, 104.6], "version")).toBe("102 to 104");
		expect(zoomSpan([101, 104], "version")).toBe("101 to 104");
	});

	// Kills a span of days read out with times, or one of hours read out as one date.
	test("names a span of days by its dates, and a shorter one by its times", () => {
		const noon = Date.UTC(2026, 9, 3, 12);
		const day = 86_400_000;
		expect(zoomSpan([noon, noon + 6 * day], "date")).toMatch(
			/^Oct \d+ to Oct \d+$/,
		);
		expect(zoomSpan([noon, noon + day / 4], "date")).toMatch(
			/^Oct \d+, \d\d:\d\d to Oct \d+, \d\d:\d\d$/,
		);
	});
});
