import { describe, expect, test } from "vitest";
import { lineLabel, rowNumbers, type UnitsPort } from "./row";

describe("lineLabel", () => {
	const LINE = {
		benchmark: "blake3",
		parameters: { simd: "avx2", threads: 1, "10": true, "9": 1.5 },
		measure: "Latency",
		metric: "p99",
	};

	// Kills the object's own key order, which puts integer keys first by number.
	test("tags every parameter in the API's key order", () => {
		expect(lineLabel(LINE, false).tags).toEqual([
			"10=true",
			"9=1.5",
			"simd=avx2",
			"threads=1",
		]);
	});

	// Kills a locale-aware sort, which puts lowercase keys before capitals.
	test("orders keys by code unit", () => {
		expect(
			lineLabel({ ...LINE, parameters: { alpha: 1, Zeta: 2 } }, false).tags,
		).toEqual(["Zeta=2", "alpha=1"]);
	});

	// Kills showing the metric on a report with one metric name, or hiding it on one with many.
	test("names the metric only when the report has more than one", () => {
		expect(lineLabel(LINE, false)).toMatchObject({
			metric: null,
			name: "blake3 10=true 9=1.5 simd=avx2 threads=1 Latency",
		});
		expect(lineLabel(LINE, true)).toMatchObject({
			metric: "p99",
			name: "blake3 10=true 9=1.5 simd=avx2 threads=1 Latency p99",
		});
	});

	// Kills a doubled space where a variant has no parameters.
	test("names a line with no parameters by its benchmark and measure", () => {
		expect(lineLabel({ ...LINE, parameters: {} }, false).name).toBe(
			"blake3 Latency",
		);
	});
});

describe("rowNumbers", () => {
	// Thousands of a unit once the smallest number reaches a thousand.
	const PORT: UnitsPort = {
		unitScale: (min, units) =>
			min >= 1000
				? { factor: 1000, symbol: `k${units}` }
				: { factor: 1, symbol: units },
		formatValue: (value, scale) => `${value / scale.factor} ${scale.symbol}`,
	};

	// Kills scaling from the value alone, which would print the limit in a unit it is under.
	test("prints the value and the limit in one unit, fit to the smaller", () => {
		expect(
			rowNumbers(
				{ value: 2000, baseline: 1000, upper_limit: 900 },
				"upper",
				"ns",
				PORT,
			),
		).toEqual({ value: "2000 ns", limit: "900 ns" });
		expect(
			rowNumbers(
				{ value: 3000, baseline: 2500, upper_limit: 2600 },
				"upper",
				"ns",
				PORT,
			),
		).toEqual({ value: "3 kns", limit: "2.6 kns" });
	});

	// Kills reading the limit from the side the threshold does not guard.
	test.each([
		["upper", "110 ns"],
		["lower", "90 ns"],
	] as const)("a threshold guarding %s shows that limit", (guard, limit) => {
		expect(
			rowNumbers(
				{ value: 100, baseline: 100, lower_limit: 90, upper_limit: 110 },
				guard,
				"ns",
				PORT,
			).limit,
		).toBe(limit);
	});

	// Kills a two-sided limit that ignores which way the value moved, or which side alerted.
	test("a two-sided threshold shows the limit the value moved toward, or the one it crossed", () => {
		const limits = { lower_limit: 90, upper_limit: 110, baseline: 100 };
		expect(
			rowNumbers({ ...limits, value: 105 }, "both", "ns", PORT).limit,
		).toBe("110 ns");
		expect(rowNumbers({ ...limits, value: 95 }, "both", "ns", PORT).limit).toBe(
			"90 ns",
		);
		expect(
			rowNumbers(
				{ ...limits, value: 105, alert: { limit: "lower" } },
				"both",
				"ns",
				PORT,
			).limit,
		).toBe("90 ns");
	});

	// Kills a limit printed for a line no threshold checks, or one whose threshold has none yet.
	test("has no limit without a threshold or before it computes one", () => {
		expect(rowNumbers({ value: 5 }, undefined, "ns", PORT)).toEqual({
			value: "5 ns",
			limit: null,
		});
		expect(rowNumbers({ value: 5 }, "upper", "ns", PORT).limit).toBeNull();
	});
});
