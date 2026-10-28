import { describe, expect, test } from "vitest";
import { iterationTag, lineLabel, rowNumbers } from "./row";

describe("iterationTag", () => {
	// Kills an iteration counted from 0, a tag on a line run once, and a count taken from the iteration shown.
	test("names the iteration a row shows of how many its line ran, when more than one", () => {
		expect(iterationTag({ iteration: 1, iterations: 3 })).toBe(
			"iteration 2 of 3",
		);
		expect(iterationTag({ iterations: 2 })).toBe("iteration 1 of 2");
		expect(iterationTag({})).toBeUndefined();
		expect(iterationTag({ iteration: 0, iterations: 1 })).toBeUndefined();
	});
});

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
	const NS = "nanoseconds (ns)";

	// Kills a limit printed in the value's tier, where a negative or zero one used to drop both to the base unit.
	test("prints the value and the limit each in its own tier", () => {
		expect(
			rowNumbers({ value: 5e9, baseline: 1e9, upper_limit: 0 }, "upper", NS),
		).toEqual({ value: "5.00 s", limit: "0" });
	});

	// Kills reading the limit from the side the threshold does not guard.
	test.each([
		["upper", "110.00 ns"],
		["lower", "90.00 ns"],
	] as const)("a threshold guarding %s shows that limit", (guard, limit) => {
		expect(
			rowNumbers(
				{ value: 100, baseline: 100, lower_limit: 90, upper_limit: 110 },
				guard,
				NS,
			).limit,
		).toBe(limit);
	});

	// Kills a two-sided limit that ignores which way the value moved, or which side alerted.
	test("a two-sided threshold shows the limit the value moved toward, or the one it crossed", () => {
		const limits = { lower_limit: 90, upper_limit: 110, baseline: 100 };
		expect(rowNumbers({ ...limits, value: 105 }, "both", NS).limit).toBe(
			"110.00 ns",
		);
		expect(rowNumbers({ ...limits, value: 95 }, "both", NS).limit).toBe(
			"90.00 ns",
		);
		expect(
			rowNumbers(
				{ ...limits, value: 105, alert: { limit: "lower" } },
				"both",
				NS,
			).limit,
		).toBe("90.00 ns");
	});

	// Kills a limit printed for a line no threshold checks, or one whose threshold has none yet.
	test("has no limit without a threshold or before it computes one", () => {
		expect(rowNumbers({ value: 5 }, undefined, NS)).toEqual({
			value: "5.00 ns",
			limit: null,
		});
		expect(rowNumbers({ value: 5 }, "upper", NS).limit).toBeNull();
	});
});
