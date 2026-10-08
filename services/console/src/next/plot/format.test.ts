import { describe, expect, test } from "vitest";
import {
	formatDate,
	formatDelta,
	formatTick,
	formatValue,
	formatWhen,
} from "./format";

describe("formatValue", () => {
	// Kills skipping the unit factor, the grouping, or the fixed two decimals.
	test("divides by the unit factor and keeps two decimals with grouping", () => {
		expect(formatValue(18_125, { factor: 1e3, symbol: "µs" })).toBe("18.13 µs");
		expect(formatValue(1_234_567.891, { factor: 1, symbol: "ns" })).toBe(
			"1,234,567.89 ns",
		);
	});

	test("leaves the unit out when the measure has no symbol", () => {
		expect(formatValue(7, { factor: 1, symbol: "" })).toBe("7.00");
	});
});

describe("formatDelta", () => {
	// Kills the sign, the rounding, or the side the threshold guards.
	test.each([
		[113.8, 100, "upper", "+13.8%", "worse"],
		[86.2, 100, "upper", "-13.8%", "better"],
		[86.2, 100, "lower", "-13.8%", "worse"],
		[113.8, 100, "lower", "+13.8%", "better"],
		[86.2, 100, "both", "-13.8%", "worse"],
		[113.8, 100, "both", "+13.8%", "worse"],
	] as const)(
		"%d against %d guarded %s reads %s %s",
		(value, baseline, guard, text, tone) => {
			expect(formatDelta(value, baseline, guard)).toEqual({ text, tone });
		},
	);

	test("names no direction under one percent", () => {
		expect(formatDelta(100.4, 100, "upper")).toEqual({
			text: "+0.4%",
			tone: null,
		});
		expect(formatDelta(99.96, 100, "upper")).toEqual({
			text: "+0.0%",
			tone: null,
		});
	});

	// Kills dividing by a negative baseline's sign, which flips the move, so a row and the readout disagree.
	test("keeps the sign of the move over a negative baseline", () => {
		expect(formatDelta(-110, -100, "upper")).toEqual({
			text: "-10.0%",
			tone: "better",
		});
	});

	test("has no delta without a baseline to divide by", () => {
		expect(formatDelta(5, 0, "upper")).toBeNull();
	});
});

describe("formatWhen", () => {
	// Kills a twelve hour clock or a dropped minute.
	test("reads month, day, and a 24 hour time", () => {
		expect(formatWhen(Date.UTC(2026, 8, 13, 21, 6), "UTC")).toBe(
			"Sep 13, 21:06",
		);
	});
});

describe("formatDate", () => {
	test("reads month and day", () => {
		expect(formatDate(Date.UTC(2026, 0, 2, 23, 59), "UTC")).toBe("Jan 2");
	});
});

describe("formatTick", () => {
	// Kills decimals that ignore the tick step.
	test.each([
		[20, 5, "20"],
		[2.5, 0.5, "2.5"],
		[0.25, 0.05, "0.25"],
		[12_500, 2500, "12,500"],
	])("labels %d at step %d as %s", (value, step, label) => {
		expect(formatTick(value, step)).toBe(label);
	});

	// A power or log scale's ticks have no common step, so each keeps its own precision.
	test("labels a tick with no common step by its own precision", () => {
		expect(formatTick(0.02, 0)).toBe("0.02");
		expect(formatTick(5000, 0)).toBe("5,000");
		expect(formatTick(123_456, 0)).toBe("123,000");
	});
});
