import { describe, expect, test } from "vitest";
import { constantParameters, keyOrder, lineNames, varyingKeys } from "./key";
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
		expect(names.map(({ text }) => text)).toEqual([
			"blake3 threads=1",
			"blake3 threads=8",
		]);
		expect(names[1]?.tags).toEqual(["threads=8"]);
	});

	// Kills naming the measure, branch, testbed, or metric when only one is drawn.
	test("adds the measure, metric, branch, or testbed only when the lines differ in it", () => {
		const one = lineNames(
			[testLine({ id: "a" }), testLine({ id: "b", benchmark: "sha256" })],
			MEASURES,
		);
		expect(one.map(({ text }) => text)).toEqual(["blake3", "sha256"]);

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
		expect(many.map(({ text }) => text)).toEqual([
			"blake3 value Latency main linux",
			"blake3 p99 Throughput feature mac",
		]);
		expect(many[1]?.detail).toEqual(["p99", "Throughput", "feature", "mac"]);
	});
});
