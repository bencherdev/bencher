import { describe, expect, test } from "vitest";
import {
	archivedBy,
	durationText,
	filterSets,
	filterSummary,
	historySpan,
	modelRows,
	modelText,
} from "./model";

const DAY = 86_400;

describe("modelText", () => {
	// Kills a test renamed for reading, such as "t-test", and a separator other than the board's.
	test("starts with the test by its API name", () => {
		expect(modelText({ test: "delta_iqr" })).toBe("delta_iqr");
	});

	// Kills a field dropped, renamed, printed out of the board's order, or printed when absent.
	test("names every field it has by the API's names, in a fixed order", () => {
		expect(
			modelText({
				test: "t_test",
				min_sample_size: 5,
				max_sample_size: 30,
				window: 90 * DAY,
				lower_boundary: 0.95,
				upper_boundary: 0.99,
			}),
		).toBe(
			"t_test · upper_boundary 0.99 · lower_boundary 0.95 · max_sample_size 30 · min_sample_size 5 · window 90 days",
		);
		expect(modelText({ test: "percentage", lower_boundary: 0.1 })).toBe(
			"percentage · lower_boundary 0.1",
		);
	});

	// Kills a boundary of zero taken for an absent one.
	test("prints a zero boundary", () => {
		expect(modelText({ test: "static", upper_boundary: 0 })).toBe(
			"static · upper_boundary 0",
		);
	});

	// Kills a threshold without a model drawn as an empty cell.
	test("says when there is no model", () => {
		expect(modelText(undefined)).toBe("no model");
	});
});

describe("modelRows", () => {
	// Kills a missing field left out of the card, where it reads as none.
	test("lists every field, the absent ones as undefined", () => {
		expect(modelRows({ test: "z_score", upper_boundary: 0.99 })).toEqual([
			["upper_boundary", "0.99"],
			["lower_boundary", undefined],
			["max_sample_size", undefined],
			["min_sample_size", undefined],
			["window", undefined],
		]);
	});
});

describe("durationText", () => {
	// Kills a unit picked before a larger one that divides it, a plural on one, and seconds printed raw.
	test("prints the largest whole unit", () => {
		expect(durationText(90 * DAY)).toBe("90 days");
		expect(durationText(28 * DAY)).toBe("4 weeks");
		expect(durationText(7 * DAY)).toBe("1 week");
		expect(durationText(DAY)).toBe("1 day");
		expect(durationText(7_200)).toBe("2 hours");
		expect(durationText(3_600)).toBe("1 hour");
		expect(durationText(60)).toBe("1 minute");
		expect(durationText(90)).toBe("90 seconds");
		expect(durationText(1)).toBe("1 second");
	});
});

describe("filterSets", () => {
	// Kills the object's own key order, insertion order with integer keys first by number, and a set merged into another.
	test("tags each set in the API's key order", () => {
		expect(
			filterSets([
				{ threads: 4, simd: "avx2", "9": 1.5, "10": true },
				{ simd: "sse4.2" },
			]),
		).toEqual([
			["10=true", "9=1.5", "simd=avx2", "threads=4"],
			["simd=sse4.2"],
		]);
	});

	// Kills an absent filter read as a filter that matches nothing.
	test("has no sets when the threshold checks every variant", () => {
		expect(filterSets(undefined)).toEqual([]);
	});
});

describe("filterSummary", () => {
	// Kills a count off by one and a plural on one.
	test("says every variant, or how many sets filter it", () => {
		expect(filterSummary(undefined)).toBe("every variant");
		expect(filterSummary([{ n: 1 }])).toBe("1 parameter set");
		expect(filterSummary([{ n: 1 }, { n: 2 }])).toBe("2 parameter sets");
	});
});

describe("historySpan", () => {
	const NOW = Date.UTC(2026, 8, 14);

	// Kills a current model given an end, and a replaced one given none.
	test("runs from set to replaced, or since set for the current model", () => {
		expect(historySpan({ created: Date.UTC(2026, 7, 30) }, NOW, "UTC")).toBe(
			"since Aug 30",
		);
		expect(
			historySpan(
				{ created: Date.UTC(2026, 7, 2), replaced: Date.UTC(2026, 7, 30) },
				NOW,
				"UTC",
			),
		).toBe("Aug 2 to Aug 30");
	});

	// Kills a date from another year printed as if it were this year's.
	test("names the year of a date outside this one", () => {
		expect(
			historySpan(
				{ created: Date.UTC(2025, 11, 30), replaced: Date.UTC(2026, 0, 2) },
				NOW,
				"UTC",
			),
		).toBe("Dec 30, 2025 to Jan 2");
	});
});

describe("archivedBy", () => {
	const dims = {
		branch: { name: "main" },
		testbed: { name: "ubuntu-latest", archived: Date.UTC(2026, 7, 9) },
		measure: { name: "Latency", archived: Date.UTC(2026, 8, 8) },
	};

	// Kills an archive told by a dimension that is not archived, or by the later of two.
	test("names the dimension archived first", () => {
		expect(archivedBy(dims)).toEqual({
			kind: "testbed",
			name: "ubuntu-latest",
			archived: Date.UTC(2026, 7, 9),
		});
	});

	// Kills a threshold whose dimensions are all active reported as archived.
	test("is undefined while every dimension is active", () => {
		expect(
			archivedBy({
				branch: dims.branch,
				testbed: { name: "ubuntu-latest" },
				measure: { name: "Latency" },
			}),
		).toBeUndefined();
	});
});
