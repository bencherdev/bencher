import { describe, expect, test } from "vitest";
import { BoundaryLimit, ModelTest } from "../../types/bencher";
import { alertFacts } from "./detail";
import { exploreLine } from "./explore";
import { THRESHOLD, alertDetail, uuid } from "./testing";

describe("alertFacts", () => {
	// Kills a heading from another table's entry, tags out of the API's key
	// order, the conventional metric named, or the line's name missing a part.
	test("names the line from its one entry in each table", () => {
		const facts = alertFacts(alertDetail());
		expect(facts).toMatchObject({
			benchmark: "blake3",
			tags: ["input=64KiB", "simd=avx2", "threads=1"],
			metric: undefined,
			measure: "Latency",
			branch: "main",
			testbed: "ubuntu-latest",
			name: "blake3 input=64KiB simd=avx2 threads=1 Latency",
		});
	});

	// Kills a metric dropped from a line that checks another one.
	test("names a metric other than the conventional one", () => {
		const detail = alertDetail();
		const line = detail.perf.lines[0];
		if (!line) {
			throw new Error("No line");
		}
		detail.perf.lines = [{ ...line, metric: "p99" }];
		expect(alertFacts(detail)).toMatchObject({
			metric: "p99",
			name: "blake3 input=64KiB simd=avx2 threads=1 Latency p99",
		});
	});

	// Kills the other side's limit, a side read from anything but the alert,
	// numbers in raw units or each in its own, and a delta not measured from the
	// baseline the threshold compared with.
	test("states the crossed limit by its side, in one unit with the value and the baseline", () => {
		expect(alertFacts(alertDetail())).toMatchObject({
			side: "upper",
			value: "20.62 µs",
			limit: "18.73 µs",
			baseline: "18.17 µs",
			delta: { text: "+13.5%", arrow: "↑", tone: "worse" },
		});
		const lower = alertDetail({
			limit: BoundaryLimit.Lower,
			value: 900,
			baseline: 1_200,
			lower_limit: 1_000,
			upper_limit: 1_400,
		});
		const model = lower.perf.models[0];
		if (!model) {
			throw new Error("No model");
		}
		lower.perf.models = [
			{ ...model, lower_boundary: 0.95, upper_boundary: 0.99 },
		];
		expect(alertFacts(lower)).toMatchObject({
			side: "lower",
			value: "900.00 ns",
			limit: "1,000.00 ns",
			baseline: "1,200.00 ns",
			delta: { text: "-25.0%", arrow: "↓", tone: "worse" },
		});
	});

	// Kills the threshold link or its words taken from the wrong model or side.
	test("names the threshold by its test and the boundary that was crossed", () => {
		expect(alertFacts(alertDetail())?.threshold).toEqual({
			uuid: THRESHOLD,
			text: "t_test · upper_boundary 0.99",
		});
		const lower = alertDetail({ limit: BoundaryLimit.Lower });
		const model = lower.perf.models[0];
		if (!model) {
			throw new Error("No model");
		}
		lower.perf.models = [
			{
				...model,
				test: ModelTest.Percentage,
				lower_boundary: 0.1,
				upper_boundary: 0.2,
			},
		];
		expect(alertFacts(lower)?.threshold?.text).toBe(
			"percentage · lower_boundary 0.1",
		);
	});

	// Kills reading a line the answer does not hold.
	test("has nothing to say of an answer with no line", () => {
		const detail = alertDetail();
		detail.perf.lines = [];
		expect(alertFacts(detail)).toBeUndefined();
	});
});

describe("exploreLine", () => {
	// Kills Explore drawing the branch's current head rather than the one the
	// alert's report ran on, or a line of another variant, measure, or metric.
	test("is the alert's line on its report's head", () => {
		expect(exploreLine(alertDetail())).toEqual({
			branch: { uuid: uuid(20), head: uuid(21) },
			testbed: { uuid: uuid(22) },
			benchmark: uuid(23),
			parameters: { simd: "avx2", input: "64KiB", threads: 1 },
			measure: uuid(25),
			metric: "value",
			alerting: true,
		});
	});
});
