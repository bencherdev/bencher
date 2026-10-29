import { describe, expect, test } from "vitest";
import { iterationTag, lineLabel, rowNumbers, shortNames } from "./row";

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

describe("shortNames", () => {
	const line = (
		benchmark: string,
		parameters: Record<string, string | number> = {},
		measure = "Latency",
		metric = "value",
	) => ({ benchmark, parameters, measure, metric });

	// Kills a control named by the whole path and every tag, the same few hundred characters on every row.
	test("names each line by what its benchmark adds and the parameters that vary", () => {
		const path = (leaf: number) =>
			`my_crate::${"module::".repeat(100)}bench_fn_name_${leaf}`;
		expect(
			shortNames(
				[
					line(path(1), { k: "a".repeat(129), n: 1 }),
					line(path(2), { k: "a".repeat(129), n: 2 }),
				],
				false,
			),
		).toEqual(["bench_fn_name_1 n=1", "bench_fn_name_2 n=2"]);
	});

	// Kills two controls with one name for two lines of one variant, or a measure on lines it cannot tell apart.
	test("adds the measure, and the metric when the list shows it, where they tell two lines apart", () => {
		expect(
			shortNames(
				[
					line("a::x", { n: 1 }),
					line("a::x", { n: 1 }, "Throughput"),
					line("a::y", { n: 1 }),
				],
				false,
			),
		).toEqual(["x Latency", "x Throughput", "y"]);
		expect(
			shortNames([line("a::x", { n: 1 }), line("a::x", { n: 1 })], false),
		).toEqual(["x", "x"]);
		expect(
			shortNames(
				[line("x", {}, "Latency", "p50"), line("x", {}, "Latency", "p99")],
				true,
			),
		).toEqual(["x p50", "x p99"]);
	});

	// Kills a lone line named by its whole path, or lines that share their benchmark named by nothing.
	test("names a lone line, and lines that share their benchmark whole, by its last segment", () => {
		expect(shortNames([line("a::b::blake3", { n: 0 })], false)).toEqual([
			"blake3",
		]);
		expect(
			shortNames(
				[line("a::blake3", { n: 0 }), line("a::blake3", { n: 1 })],
				false,
			),
		).toEqual(["blake3 n=0", "blake3 n=1"]);
	});
});
