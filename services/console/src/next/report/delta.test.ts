import { describe, expect, test } from "vitest";
import { deltaOf, guardOf } from "./delta";

describe("guardOf", () => {
	// Kills reading one boundary for the other, or dropping the two-sided case.
	test.each([
		[{ upper_boundary: 0.99 }, "upper"],
		[{ lower_boundary: 0.99 }, "lower"],
		[{ lower_boundary: 0.95, upper_boundary: 0.99 }, "both"],
	] as const)("a model with %o guards %s", (model, guard) => {
		expect(guardOf(model)).toBe(guard);
	});

	// Kills treating a line no threshold checked as guarded.
	test("a line no threshold checked has no guarded side", () => {
		expect(guardOf(undefined)).toBeUndefined();
	});
});

describe("deltaOf", () => {
	// Kills the sign, the rounding, the arrow, or the side the threshold guards.
	test.each([
		[113.8, 100, "upper", "+13.8%", "↑", "worse"],
		[86.2, 100, "upper", "-13.8%", "↓", "better"],
		[86.2, 100, "lower", "-13.8%", "↓", "worse"],
		[113.8, 100, "lower", "+13.8%", "↑", "better"],
		[86.2, 100, "both", "-13.8%", "↓", "worse"],
		[113.8, 100, "both", "+13.8%", "↑", "worse"],
	] as const)(
		"%d against %d guarded %s reads %s %s %s",
		(value, baseline, guard, text, arrow, word) => {
			expect(deltaOf(value, baseline, guard)).toEqual({ text, arrow, word });
		},
	);

	// Kills dividing by a signed baseline, which flips the direction the API sorts by.
	test("a negative baseline keeps the guarded side", () => {
		expect(deltaOf(-80, -100, "upper")).toEqual({
			text: "+20.0%",
			arrow: "↑",
			word: "worse",
		});
		expect(deltaOf(-120, -100, "lower")).toEqual({
			text: "-20.0%",
			arrow: "↓",
			word: "worse",
		});
	});

	// Kills naming a direction for noise, or a cutoff other than one percent.
	test("names no direction under one percent", () => {
		expect(deltaOf(100.9, 100, "upper")).toEqual({
			text: "+0.9%",
			arrow: null,
			word: null,
		});
		expect(deltaOf(101, 100, "upper")).toEqual({
			text: "+1.0%",
			arrow: "↑",
			word: "worse",
		});
		expect(deltaOf(99.96, 100, "upper")).toEqual({
			text: "+0.0%",
			arrow: null,
			word: null,
		});
	});

	// Kills a delta with nothing to compare against.
	test.each([
		["no threshold", 5, 4, undefined],
		["no baseline", 5, undefined, "upper"],
		["a null baseline", 5, null, "upper"],
		["a zero baseline", 5, 0, "upper"],
	] as const)("is blank with %s", (_, value, baseline, guard) => {
		expect(deltaOf(value, baseline, guard)).toBeNull();
	});
});
