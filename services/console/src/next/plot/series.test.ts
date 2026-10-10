import { describe, expect, test } from "vitest";
import { DOTTED, layoutOf, lineDash, lineStyles } from "./series";
import { testLine } from "./testing";

const lines = (count: number, measure = 0) =>
	Array.from({ length: count }, (_, index) =>
		testLine({ id: `l${index}`, parameters: { threads: index }, measure }),
	);

describe("lineStyles", () => {
	// Kills a slot that is not the line's position mod 8.
	test("gives each line the next of eight slots", () => {
		expect(lineStyles(lines(10), "single").map(({ slot }) => slot)).toEqual([
			1, 2, 3, 4, 5, 6, 7, 8, 1, 2,
		]);
	});

	// Kills a shape that does not change once the colors repeat.
	test("changes the point shape each time the colors repeat", () => {
		const shapes = lineStyles(lines(33), "single").map(({ shape }) => shape);
		expect(shapes.slice(0, 8)).toEqual(Array(8).fill("circle"));
		expect(shapes.slice(8, 16)).toEqual(Array(8).fill("hollow"));
		expect(shapes.slice(16, 24)).toEqual(Array(8).fill("square"));
		expect(shapes.slice(24, 32)).toEqual(Array(8).fill("triangle"));
		expect(shapes[32]).toBe("circle");
	});

	// Both measures of one variant read as one series in two places.
	test("gives both measures of one variant one color", () => {
		const styles = lineStyles(
			[
				testLine({ id: "a-lat", parameters: { threads: 1 }, measure: 0 }),
				testLine({ id: "b-lat", parameters: { threads: 2 }, measure: 0 }),
				testLine({ id: "a-thr", parameters: { threads: 1 }, measure: 1 }),
				testLine({ id: "b-thr", parameters: { threads: 2 }, measure: 1 }),
			],
			"dual",
		);
		expect(styles.map(({ slot }) => slot)).toEqual([1, 2, 1, 2]);
	});

	test("tells apart the same variant on another branch, testbed, or metric", () => {
		const styles = lineStyles(
			[
				testLine({ id: "a" }),
				testLine({ id: "b", branch: "feature" }),
				testLine({ id: "c", testbed: "mac" }),
				testLine({ id: "d", metric: "p99" }),
				testLine({ id: "e", parameters: { simd: "avx2" } }),
			],
			"single",
		);
		expect(styles.map(({ slot }) => slot)).toEqual([1, 2, 3, 4, 5]);
	});

	// Kills putting the second measure on the left axis or in the first plot.
	test("puts the second measure on the right axis of a dual layout", () => {
		const styles = lineStyles(
			[testLine({ id: "a", measure: 3 }), testLine({ id: "b", measure: 1 })],
			"dual",
		);
		expect(styles.map(({ axis, panel }) => [axis, panel])).toEqual([
			[1, 0],
			[0, 0],
		]);
	});

	test("gives each measure its own plot when stacked", () => {
		const styles = lineStyles(
			[
				testLine({ id: "a", measure: 2 }),
				testLine({ id: "b", measure: 0 }),
				testLine({ id: "c", measure: 1 }),
			],
			"stacked",
		);
		expect(styles.map(({ axis, panel }) => [axis, panel])).toEqual([
			[0, 2],
			[0, 0],
			[0, 1],
		]);
	});
});

describe("lineDash", () => {
	// Dotted strokes mean the second axis and nothing else.
	test("dots only a line on the right axis", () => {
		const styles = lineStyles(
			[testLine({ id: "a", measure: 0 }), testLine({ id: "b", measure: 1 })],
			"dual",
		);
		expect(styles.map(lineDash)).toEqual([undefined, DOTTED]);
	});

	test("draws every line solid when stacked", () => {
		const styles = lineStyles(
			[testLine({ id: "a", measure: 0 }), testLine({ id: "b", measure: 1 })],
			"stacked",
		);
		expect(styles.map(lineDash)).toEqual([undefined, undefined]);
	});
});

describe("layoutOf", () => {
	test("draws one measure on one axis", () => {
		expect(layoutOf(1, "stacked")).toBe("single");
	});

	test("draws two measures on two axes unless stacked is asked for", () => {
		expect(layoutOf(2, "dual")).toBe("dual");
		expect(layoutOf(2, "stacked")).toBe("stacked");
	});

	// Kills allowing a dual axis past two measures.
	test("stacks three measures or more", () => {
		expect(layoutOf(3, "dual")).toBe("stacked");
	});
});
