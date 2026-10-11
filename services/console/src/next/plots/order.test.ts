import { describe, expect, test } from "vitest";
import { moveIndex, placed, positionIndex, positionOf } from "./order";

const ORDER = ["a", "b", "c", "d"];

describe("moveIndex", () => {
	// Kills an index that counts the moved plot among the others (down by two),
	// a bottom past the end, and a move off either end.
	test("send a plot to its place among the others", () => {
		expect(moveIndex(ORDER, "b", "up")).toBe(0);
		expect(moveIndex(ORDER, "b", "down")).toBe(2);
		expect(moveIndex(ORDER, "c", "top")).toBe(0);
		expect(moveIndex(ORDER, "b", "bottom")).toBe(3);
		expect(moveIndex(ORDER, "a", "up")).toBeUndefined();
		expect(moveIndex(ORDER, "a", "top")).toBeUndefined();
		expect(moveIndex(ORDER, "d", "down")).toBeUndefined();
		expect(moveIndex(ORDER, "d", "bottom")).toBeUndefined();
		expect(moveIndex(ORDER, "z", "up")).toBeUndefined();
	});

	// Kills a local order that disagrees with where the API puts the plot.
	test("place a plot where the API does", () => {
		const move = (plot: string, to: "up" | "down" | "top" | "bottom") =>
			placed(ORDER, plot, moveIndex(ORDER, plot, to) ?? -1);
		expect(move("b", "up")).toEqual(["b", "a", "c", "d"]);
		expect(move("b", "down")).toEqual(["a", "c", "b", "d"]);
		expect(move("c", "top")).toEqual(["c", "a", "b", "d"]);
		expect(move("a", "bottom")).toEqual(["b", "c", "d", "a"]);
	});
});

describe("positionIndex", () => {
	// Kills an after that lands before its plot, a bottom that counts the plot
	// itself, and an unknown plot that moves the pin to the top.
	test("place a pin at the top, at the bottom, or after another", () => {
		expect(positionIndex(ORDER, "b", { at: "top" })).toBe(0);
		expect(positionIndex(ORDER, "b", { at: "bottom" })).toBe(3);
		expect(positionIndex(ORDER, "b", { at: "after", plot: "c" })).toBe(2);
		expect(positionIndex(ORDER, "d", { at: "after", plot: "a" })).toBe(1);
		expect(positionIndex(ORDER, "b", { at: "after", plot: "gone" })).toBe(3);
		expect(
			placed(ORDER, "b", positionIndex(ORDER, "b", { at: "after", plot: "c" })),
		).toEqual(["a", "c", "b", "d"]);
	});

	// Kills a dialog that opens a pin anywhere but where it is.
	test("open at the place a pin holds", () => {
		expect(positionOf(ORDER, "a")).toEqual({ at: "top" });
		expect(positionOf(ORDER, "c")).toEqual({ at: "after", plot: "b" });
		expect(positionIndex(ORDER, "c", positionOf(ORDER, "c"))).toBe(2);
		expect(positionIndex(ORDER, "d", positionOf(ORDER, "d"))).toBe(3);
	});
});
