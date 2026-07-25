import { describe, expect, test } from "vitest";
import { percentDifference, percentOf } from "./percent";

describe("percentDifference", () => {
	test("a value that drops to zero is a 100% improvement", () => {
		expect(percentDifference(0.0, 3_069_448.0)).toBe(-100.0);
	});

	test("a value below the baseline is a negative percent", () => {
		expect(percentDifference(50.0, 100.0)).toBe(-50.0);
	});

	test("a value above the baseline is a positive percent", () => {
		expect(percentDifference(150.0, 100.0)).toBe(50.0);
	});

	test("a value equal to the baseline is no change", () => {
		expect(percentDifference(100.0, 100.0)).toBe(0.0);
	});

	test("a zero baseline has no percent difference", () => {
		expect(percentDifference(0.0, 0.0)).toBe(0.0);
		expect(percentDifference(100.0, 0.0)).toBe(0.0);
	});

	test("a non-finite value or baseline has no percent difference", () => {
		expect(percentDifference(Number.POSITIVE_INFINITY, 100.0)).toBe(0.0);
		expect(percentDifference(100.0, Number.POSITIVE_INFINITY)).toBe(0.0);
		expect(percentDifference(Number.NaN, 100.0)).toBe(0.0);
	});
});

describe("percentOf", () => {
	test("a zero numerator is zero percent of the denominator", () => {
		expect(percentOf(0.0, 100.0)).toBe(0.0);
	});

	test("a numerator is its share of the denominator", () => {
		expect(percentOf(50.0, 100.0)).toBe(50.0);
		expect(percentOf(100.0, 100.0)).toBe(100.0);
		expect(percentOf(200.0, 100.0)).toBe(200.0);
	});

	test("a zero denominator has no percent", () => {
		expect(percentOf(100.0, 0.0)).toBe(0.0);
		expect(percentOf(0.0, 0.0)).toBe(0.0);
	});

	test("a non-finite numerator or denominator has no percent", () => {
		expect(percentOf(Number.POSITIVE_INFINITY, 100.0)).toBe(0.0);
		expect(percentOf(100.0, Number.POSITIVE_INFINITY)).toBe(0.0);
		expect(percentOf(Number.NaN, 100.0)).toBe(0.0);
	});
});
