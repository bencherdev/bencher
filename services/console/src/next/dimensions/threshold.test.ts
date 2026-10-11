import { describe, expect, test } from "vitest";
import type { JsonThreshold } from "../../types/bencher";
import { thresholdLinkName } from "./threshold";

const threshold = (fields: Partial<JsonThreshold> = {}) =>
	({
		branch: { name: "main" },
		testbed: { name: "ubuntu-latest" },
		measure: { name: "Latency" },
		...fields,
	}) as unknown as JsonThreshold;

describe("thresholdLinkName", () => {
	// Kills a name that leads with anything but the text the link shows.
	test("starts with the text the link shows, then which threshold it opens", () => {
		expect(thresholdLinkName("ubuntu-latest", threshold())).toBe(
			"ubuntu-latest, threshold for Latency on main, ubuntu-latest",
		);
	});

	// Kills two thresholds on one measure that differ only by their parameter
	// sets, or only by their metric, sharing a name.
	test("tells apart thresholds that differ only by parameter sets or metric", () => {
		const names = [
			threshold({ parameters: [{ n: 1 }] }),
			threshold({ parameters: [{ n: 2, simd: "avx2" }, { n: 4 }] }),
			threshold({ metric: "p99" }),
			threshold({ metric: "value" }),
		].map((each) => thresholdLinkName("ubuntu-latest", each));
		expect(names).toEqual([
			"ubuntu-latest, threshold for Latency on main, ubuntu-latest, n=1",
			"ubuntu-latest, threshold for Latency on main, ubuntu-latest, n=2 simd=avx2 or n=4",
			"ubuntu-latest, threshold for Latency on main, ubuntu-latest, metric p99",
			"ubuntu-latest, threshold for Latency on main, ubuntu-latest",
		]);
	});

	// Kills a name that tells the metric before the parameter sets.
	test("tells the parameter sets before the metric", () => {
		expect(
			thresholdLinkName(
				"ubuntu-latest",
				threshold({ parameters: [{ n: 1 }], metric: "p99" }),
			),
		).toBe(
			"ubuntu-latest, threshold for Latency on main, ubuntu-latest, n=1, metric p99",
		);
	});
});
