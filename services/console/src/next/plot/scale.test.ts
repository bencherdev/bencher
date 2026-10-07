import { describe, expect, test } from "vitest";
import { transform, yRange, yScale, yTicks } from "./scale";

const exponentOf = (scale: ReturnType<typeof yScale>) => {
	if (scale.kind !== "pow") {
		throw new Error(`Expected a power scale, got ${scale.kind}`);
	}
	return scale.exponent;
};

describe("yScale", () => {
	test("linear stays linear for a large spread", () => {
		expect(yScale("linear", 1, 1_000_000).kind).toBe("linear");
	});

	test("log is a log scale when the minimum is positive", () => {
		expect(yScale("log", 1, 1000).kind).toBe("log");
		expect(yScale("log", 5, 5).kind).toBe("log");
	});

	// The logarithm is undefined at zero and below, so log falls back to auto.
	test("log falls back to auto at a minimum of zero", () => {
		expect(exponentOf(yScale("log", 0, 100))).toBeCloseTo(1 / 3);
	});

	test("log falls back to auto at a negative minimum, which is linear", () => {
		expect(yScale("log", -5, 100).kind).toBe("linear");
	});

	// Kills moving the tenfold cutoff.
	test("auto is linear up to a tenfold spread", () => {
		expect(yScale("auto", 10, 50).kind).toBe("linear");
		expect(yScale("auto", 10, 100).kind).toBe("linear");
	});

	// Kills a wrong exponent formula or a missing floor.
	test("auto adapts the exponent to the spread, never below one third", () => {
		expect(exponentOf(yScale("auto", 1, 100))).toBeCloseTo(1 / 2);
		expect(exponentOf(yScale("auto", 1, 1000))).toBeCloseTo(1 / 3);
		expect(exponentOf(yScale("auto", 1, 1_000_000))).toBeCloseTo(1 / 3);
	});

	test("auto is linear when every value is zero", () => {
		expect(yScale("auto", 0, 0).kind).toBe("linear");
	});
});

describe("transform", () => {
	// Kills a power transform that loses the sign below zero.
	test("round trips a power scale through negative values", () => {
		const { fwd, bwd } = transform({ kind: "pow", exponent: 1 / 3 });
		expect(fwd(-8)).toBeCloseTo(-2);
		expect(bwd(fwd(-8))).toBeCloseTo(-8);
		expect(bwd(fwd(27))).toBeCloseTo(27);
	});

	test("maps equal ratios to equal steps on a log scale", () => {
		const { fwd } = transform({ kind: "log" });
		expect(fwd(100) - fwd(10)).toBeCloseTo(fwd(1000) - fwd(100));
	});
});

describe("yRange", () => {
	test.each([
		["linear", 3, 97],
		["auto", 3, 97],
		["auto", 1, 1000],
		["log", 3, 97],
	] as const)(
		"a %s range covers %d to %d with room on both sides",
		(mode, min, max) => {
			const [lo, hi] = yRange(yScale(mode, min, max), min, max);
			expect(lo).toBeLessThan(min);
			expect(hi).toBeGreaterThan(max);
		},
	);

	// Kills padding in value space instead of the scale's own space.
	test("pads a power scale in its own space", () => {
		const pow = { kind: "pow", exponent: 1 / 3 } as const;
		const { fwd } = transform(pow);
		const [lo, hi] = yRange(pow, 8, 1000);
		const span = fwd(1000) - fwd(8);
		expect((fwd(8) - fwd(lo)) / span).toBeCloseTo(0.16);
		expect((fwd(hi) - fwd(1000)) / span).toBeCloseTo(0.18);
	});

	// A quantity that is never negative never shows a negative axis.
	test("stops at zero below values that start at zero", () => {
		expect(yRange(yScale("linear", 0, 10), 0, 10)[0]).toBe(0);
		expect(yRange(yScale("auto", 0, 1000), 0, 1000)[0]).toBe(0);
	});

	test("opens a range around a single value", () => {
		const [lo, hi] = yRange(yScale("auto", 5, 5), 5, 5);
		expect(lo).toBeLessThan(5);
		expect(hi).toBeGreaterThan(5);
	});
});

describe("yTicks", () => {
	// Kills a step that is not 1, 2, 2.5, or 5 times a power of ten.
	test("steps a linear scale by a nice number", () => {
		expect(yTicks({ kind: "linear" }, 0.3, 9.8, 6)).toEqual({
			ticks: [2, 4, 6, 8],
			step: 2,
		});
		expect(yTicks({ kind: "linear" }, 0.3, 9.8, 5)).toEqual({
			ticks: [2.5, 5, 7.5],
			step: 2.5,
		});
	});

	// Kills ticks spaced evenly in value space, which bunch at the top of a power scale.
	test("spreads ticks over a power scale in its own space", () => {
		const { ticks, step } = yTicks(
			{ kind: "pow", exponent: 1 / 3 },
			1,
			1000,
			4,
		);
		expect(step).toBe(0);
		expect(ticks).toEqual([10, 100, 200, 500]);
	});

	test("puts log ticks on powers of ten, thinned to the count", () => {
		expect(yTicks({ kind: "log" }, 0.8, 12_000, 5).ticks).toEqual([
			1, 10, 100, 1000, 10_000,
		]);
		expect(yTicks({ kind: "log" }, 0.8, 1.2e6, 3).ticks).toEqual([
			1, 1000, 1e6,
		]);
	});

	test("falls back to linear ticks when too few nice values fall inside", () => {
		const { ticks, step } = yTicks({ kind: "log" }, 11, 14, 5);
		expect(step).toBeGreaterThan(0);
		expect(ticks.length).toBeGreaterThanOrEqual(3);
		for (const tick of ticks) {
			expect(tick).toBeGreaterThanOrEqual(11);
			expect(tick).toBeLessThanOrEqual(14);
		}
	});
});

describe("yTicks over many ranges", () => {
	const random = (() => {
		let state = 7;
		return () => {
			state = (Math.imul(state, 1664525) + 1013904223) >>> 0;
			return state / 4294967296;
		};
	})();

	// Kills ticks that fall outside the axis, as nice values past either end would.
	test("puts every tick inside the range, for every kind of scale", () => {
		for (let run = 0; run < 400; run++) {
			const magnitude = 10 ** Math.floor(random() * 16 - 6);
			const min = random() * magnitude;
			const max = min + random() * magnitude * 10 ** Math.floor(random() * 4);
			for (const mode of ["auto", "linear", "log"] as const) {
				const scale = yScale(mode, min, max);
				const [lo, hi] = yRange(scale, min, max);
				const slack = (hi - lo) * 1e-9;
				for (const count of [3, 4, 5]) {
					for (const tick of yTicks(scale, lo, hi, count).ticks) {
						expect(tick, `${mode} ${lo} ${hi}`).toBeGreaterThanOrEqual(
							lo - slack,
						);
						expect(tick, `${mode} ${lo} ${hi}`).toBeLessThanOrEqual(hi + slack);
					}
				}
			}
		}
	});
});
