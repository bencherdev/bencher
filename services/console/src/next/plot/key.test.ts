import { describe, expect, test } from "vitest";
import { distinct, varyingKeys } from "./distinct";
import { constantParameters, keyOrder, lineNames } from "./key";
import { testLine } from "./testing";

const MEASURES = [
	{ name: "Latency", units: "nanoseconds (ns)" },
	{ name: "Throughput", units: "operations / second (ops/s)" },
];

describe("keyOrder", () => {
	// Kills an unstable sort or alerting lines left in place.
	test("puts alerting lines first and keeps the given order otherwise", () => {
		const lines = [
			testLine({ id: "a" }),
			testLine({ id: "b", alerting: true }),
			testLine({ id: "c" }),
			testLine({ id: "d", alerting: true }),
			testLine({ id: "e" }),
		];
		expect(keyOrder(lines)).toEqual([1, 3, 0, 2, 4]);
	});
});

describe("varyingKeys", () => {
	// Kills naming a key that every line shares.
	test("names the keys whose values differ across the lines", () => {
		const lines = [
			testLine({ id: "a", parameters: { input_bytes: 65536, threads: 1 } }),
			testLine({ id: "b", parameters: { input_bytes: 65536, threads: 4 } }),
		];
		expect(varyingKeys(lines)).toEqual(["threads"]);
	});

	test("counts a key some lines lack as varying", () => {
		const lines = [
			testLine({ id: "a", parameters: { simd: "avx2", threads: 1 } }),
			testLine({ id: "b", parameters: { simd: "avx2" } }),
		];
		expect(varyingKeys(lines)).toEqual(["threads"]);
	});

	// Kills comparing values by their text, which makes 1 and "1" one value.
	test("tells a number from the same text", () => {
		const lines = [
			testLine({ id: "a", parameters: { size: 1 } }),
			testLine({ id: "b", parameters: { size: "1" } }),
		];
		expect(varyingKeys(lines)).toEqual(["size"]);
	});
});

describe("constantParameters", () => {
	test("lists the parameters every line carries with one value", () => {
		const lines = [
			testLine({ id: "a", parameters: { input_bytes: 65536, threads: 1 } }),
			testLine({ id: "b", parameters: { input_bytes: 65536, threads: 4 } }),
			testLine({ id: "c", parameters: { input_bytes: 65536 } }),
		];
		expect(constantParameters(lines)).toEqual(["input_bytes=65536"]);
	});

	test("lists nothing for one line, whose key entry names it in full", () => {
		expect(
			constantParameters([testLine({ id: "a", parameters: { threads: 1 } })]),
		).toEqual([]);
	});
});

const path = (leaf: number) =>
	`my_crate::${Array.from({ length: 80 }, (_, at) => `module_${at}`).join("::")}::bench_fn_name_${String(leaf).padStart(4, "0")}`;

describe("distinct", () => {
	// Kills a prefix cut inside the last segment, which merges names that differ only in its end.
	test("gives 64 paths that differ in the last segment 64 distinct rests", () => {
		const names = Array.from({ length: 64 }, (_, leaf) => path(leaf));
		const { prefix, rests } = distinct(names);
		expect(rests[2]).toBe("bench_fn_name_0002");
		expect(new Set(rests).size).toBe(64);
		expect(rests.map((rest) => prefix + rest)).toEqual(names);
	});

	// Kills a character prefix that is no segment: `bench_fn_name_000` would leave rests of one digit.
	test("gives names with no shared segment no prefix", () => {
		expect(distinct(["bench_fn_name_0001", "bench_fn_name_0002"])).toEqual({
			prefix: "",
			rests: ["bench_fn_name_0001", "bench_fn_name_0002"],
		});
		expect(distinct(["alpha::x", "beta::x"]).prefix).toBe("");
	});

	test("cuts after a slash as after a double colon", () => {
		expect(distinct(["release/q4/leaf-000", "release/q4/leaf-001"])).toEqual({
			prefix: "release/q4/",
			rests: ["leaf-000", "leaf-001"],
		});
	});

	// Kills an empty rest for a name the others extend.
	test("leaves every name a rest when one is the start of another", () => {
		expect(distinct(["a::b", "a::b::c"])).toEqual({
			prefix: "a::",
			rests: ["b", "b::c"],
		});
		expect(distinct(["release/", "release/x"]).prefix).toBe("");
	});

	test("shares one name whole, and a lone name not at all", () => {
		expect(distinct(["a::b", "a::b"])).toEqual({
			prefix: "a::b",
			rests: ["", ""],
		});
		expect(distinct(["a::b"])).toEqual({ prefix: "", rests: ["a::b"] });
	});
});

describe("lineNames", () => {
	// Kills tags for keys that do not vary, or a missing tag for one that does.
	test("names a line by its benchmark and the parameters that vary", () => {
		const names = lineNames(
			[
				testLine({ id: "a", parameters: { input_bytes: 64, threads: 1 } }),
				testLine({ id: "b", parameters: { input_bytes: 64, threads: 8 } }),
			],
			MEASURES,
		);
		expect(names.names.map(({ text }) => text)).toEqual(["1", "8"]);
		expect(names.shared).toEqual({
			benchmark: "blake3",
			parameters: ["input_bytes=64", "threads="],
		});
	});

	// Kills a bare value for the key some line lacks, and a line named by nothing.
	test("keeps the key beside its value when a line lacks it, and names that line by its benchmark", () => {
		const names = lineNames(
			[
				testLine({
					id: "a",
					benchmark: "hash::blake3",
					parameters: { threads: 1 },
				}),
				testLine({ id: "b", benchmark: "hash::blake3" }),
			],
			MEASURES,
		);
		expect(names.names.map(({ text }) => text)).toEqual([
			"threads=1",
			"blake3",
		]);
		expect(names.shared).toEqual({ benchmark: "hash::blake3", parameters: [] });
	});

	// Kills a key moved to the head while two keys vary, which leaves values no one can read.
	test("keeps each key beside its value when two keys vary", () => {
		const names = lineNames(
			[
				testLine({ id: "a", parameters: { simd: "avx2", threads: 1 } }),
				testLine({ id: "b", parameters: { simd: "sse4.2", threads: 8 } }),
			],
			MEASURES,
		);
		expect(names.names[1]?.tags).toEqual(["simd=sse4.2", "threads=8"]);
		expect(names.shared.parameters).toEqual([]);
	});

	// The 8 by 8 stress plot: the shared path heads the key, each entry its benchmark's end and value.
	test("names 8 benchmarks by 8 variants by what tells them apart", () => {
		const lines = Array.from({ length: 64 }, (_, at) =>
			testLine({
				id: `${at}`,
				benchmark: path(3000 + Math.floor(at / 8)),
				parameters: { key: `${at % 8}-value` },
			}),
		);
		const names = lineNames(lines, MEASURES);
		expect(names.names[9]?.text).toBe("bench_fn_name_3001 1-value");
		expect(new Set(names.names.map(({ text }) => text)).size).toBe(64);
		expect(names.shared).toEqual({
			benchmark: path(0).slice(0, -"::bench_fn_name_0000".length),
			parameters: ["key="],
		});
	});

	// Kills naming the measure, branch, testbed, or metric when only one is drawn.
	test("adds the measure, metric, branch, or testbed only when the lines differ in it", () => {
		const one = lineNames(
			[testLine({ id: "a" }), testLine({ id: "b", benchmark: "sha256" })],
			MEASURES,
		);
		expect(one.names.map(({ text }) => text)).toEqual(["blake3", "sha256"]);

		const many = lineNames(
			[
				testLine({ id: "a" }),
				testLine({
					id: "b",
					measure: 1,
					metric: "p99",
					branch: "feature",
					testbed: "mac",
				}),
			],
			MEASURES,
		);
		expect(many.names.map(({ text }) => text)).toEqual([
			"value Latency main linux",
			"p99 Throughput feature mac",
		]);
		expect(many.names[1]?.detail).toEqual([
			"p99",
			"Throughput",
			"feature",
			"mac",
		]);
	});
});
