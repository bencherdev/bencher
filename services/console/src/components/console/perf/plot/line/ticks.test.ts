import { describe, expect, test } from "vitest";
import { get_x_ticks } from "./ticks";

describe("get_x_ticks", () => {
	test("returns all distinct values sorted when they fit the width", () => {
		expect(get_x_ticks([3, 1, 2], 800)).toEqual([1, 2, 3]);
	});

	test("dedupes version numbers shared across branches", () => {
		expect(get_x_ticks([1, 1, 2, 2, 3, 3], 800)).toEqual([1, 2, 3]);
	});

	test("sorts numerically, not lexicographically", () => {
		expect(get_x_ticks([100, 20, 3], 800)).toEqual([3, 20, 100]);
	});

	test("thins to roughly one tick per 80px of width", () => {
		const versions = Array.from({ length: 100 }, (_, i) => i + 1);
		const ticks = get_x_ticks(versions, 800);
		expect(ticks).toEqual([1, 11, 21, 31, 41, 51, 61, 71, 81, 91]);
	});

	test("every tick is a member of the input values", () => {
		// A point scale is only defined on its domain,
		// so every tick must be an actual domain value.
		const versions = Array.from({ length: 257 }, (_, i) => i * 3 + 7);
		const ticks = get_x_ticks(versions, 1170);
		expect(ticks.length).toBeGreaterThan(2);
		const domain = new Set(versions);
		for (const tick of ticks) {
			expect(domain.has(tick)).toBe(true);
		}
	});

	test("keeps at least two ticks on narrow plots", () => {
		const versions = Array.from({ length: 50 }, (_, i) => i + 1);
		expect(get_x_ticks(versions, 100)).toEqual([1, 26]);
	});

	test("ignores missing version numbers", () => {
		expect(get_x_ticks([2, undefined, 1, Number.NaN], 800)).toEqual([1, 2]);
	});

	test("returns an empty array for no data", () => {
		expect(get_x_ticks([], 800)).toEqual([]);
	});
});
