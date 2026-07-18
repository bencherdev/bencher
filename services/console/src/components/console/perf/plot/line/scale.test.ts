import { describe, expect, test } from "vitest";
import { YAxis } from "../../../../../types/bencher";
import { type YScale, get_y_scale } from "./scale";

// A d3 pow scale exposes `exponent()`; a d3 log scale exposes `base()`.
const expectPow = (scale: YScale) => {
	if (!("exponent" in scale)) {
		throw new Error("Expected a pow scale");
	}
	return scale;
};

const expectLog = (scale: YScale) => {
	if (!("base" in scale)) {
		throw new Error("Expected a log scale");
	}
	return scale;
};

describe("get_y_scale", () => {
	test("Linear returns a pow scale with exponent 1", () => {
		const scale = expectPow(get_y_scale(YAxis.Linear, 1, 100));
		expect(scale.exponent()).toBe(1);
	});

	test("Linear stays exponent 1 even for a large data spread", () => {
		const scale = expectPow(get_y_scale(YAxis.Linear, 1, 1_000_000));
		expect(scale.exponent()).toBe(1);
	});

	test("Log returns a log scale when the minimum is positive", () => {
		const scale = expectLog(get_y_scale(YAxis.Log, 1, 1000));
		// The domain is niced to cover the data
		const [lower, upper] = scale.domain();
		expect(lower).toBeLessThanOrEqual(1);
		expect(upper).toBeGreaterThanOrEqual(1000);
		// Equal ratios map to equal distances on a log scale
		const delta = scale(10) - scale(1);
		expect(scale(100) - scale(10)).toBeCloseTo(delta);
		expect(scale(1000) - scale(100)).toBeCloseTo(delta);
	});

	test("Log handles a degenerate domain where min equals max", () => {
		// d3 nices a collapsed log domain out to the surrounding powers of ten,
		// so a plot where every value is identical still renders sanely.
		const scale = expectLog(get_y_scale(YAxis.Log, 5, 5));
		const [lower, upper] = scale.domain();
		expect(lower).toBeLessThan(upper);
		expect(lower).toBeLessThanOrEqual(5);
		expect(upper).toBeGreaterThanOrEqual(5);
		expect(Number.isFinite(scale(5))).toBe(true);
	});

	test("Log falls back to the auto scale when the minimum is zero", () => {
		const scale = expectPow(get_y_scale(YAxis.Log, 0, 100));
		// max / min is Infinity, so the auto exponent bottoms out at 1/3
		expect(scale.exponent()).toBeCloseTo(1 / 3);
	});

	test("Log falls back to the auto scale when the minimum is negative", () => {
		const scale = expectPow(get_y_scale(YAxis.Log, -5, 100));
		// max / min is negative, which is below the 10x cutoff, so linear
		expect(scale.exponent()).toBe(1);
	});

	test("Auto returns a linear scale for a spread under 10x", () => {
		const scale = expectPow(get_y_scale(YAxis.Auto, 10, 50));
		expect(scale.exponent()).toBe(1);
	});

	test("Auto adapts the exponent to the data spread at 10x and above", () => {
		// 1 / log10(100) = 1/2
		const hundred = expectPow(get_y_scale(YAxis.Auto, 1, 100));
		expect(hundred.exponent()).toBeCloseTo(1 / 2);
		// 1 / log10(1000) = 1/3
		const thousand = expectPow(get_y_scale(YAxis.Auto, 1, 1000));
		expect(thousand.exponent()).toBeCloseTo(1 / 3);
		// The exponent never drops below 1/3
		const million = expectPow(get_y_scale(YAxis.Auto, 1, 1_000_000));
		expect(million.exponent()).toBeCloseTo(1 / 3);
	});

	test("the niced domain covers the min and max", () => {
		for (const y_axis of [YAxis.Auto, YAxis.Linear, YAxis.Log]) {
			const scale = get_y_scale(y_axis, 3, 97);
			const [lower, upper] = scale.domain();
			expect(lower).toBeLessThanOrEqual(3);
			expect(upper).toBeGreaterThanOrEqual(97);
		}
	});
});
