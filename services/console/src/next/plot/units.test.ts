import { describe, expect, test } from "vitest";
import { unitScale } from "./units";

describe("unitScale", () => {
	// Kills a shifted tier boundary: each boundary value starts the next tier.
	test.each([
		[999, 1, "ns"],
		[1000, 1e3, "µs"],
		[999_999, 1e3, "µs"],
		[1e6, 1e6, "ms"],
		[1e9, 1e9, "s"],
		[59.9e9, 1e9, "s"],
		[60e9, 60e9, "m"],
		[3.6e12, 3.6e12, "h"],
	])("scales %d nanoseconds by %d to %s", (min, factor, symbol) => {
		expect(unitScale(min, "nanoseconds (ns)")).toEqual({ factor, symbol });
	});

	test.each([
		[59, 1, "s"],
		[60, 60, "m"],
		[3600, 3600, "h"],
	])("scales %d seconds by %d to %s", (min, factor, symbol) => {
		expect(unitScale(min, "seconds (s)")).toEqual({ factor, symbol });
	});

	test.each([
		[999, 1, "B"],
		[2.5e6, 1e6, "MB"],
		[1e15, 1e15, "PB"],
	])("scales %d bytes by %d to %s", (min, factor, symbol) => {
		expect(unitScale(min, "bytes (B)")).toEqual({ factor, symbol });
	});

	// Kills reading the symbol from the wrong delimiter or dropping the power suffix.
	test("names any other unit by the symbol in its parentheses with a power of ten", () => {
		expect(unitScale(42, "instructions (ins)")).toEqual({
			factor: 1,
			symbol: "ins",
		});
		expect(unitScale(5e6, "widgets (w)")).toEqual({
			factor: 1e6,
			symbol: "w x 1e6",
		});
	});

	test("leaves a unit with no symbol bare until it needs a power of ten", () => {
		expect(unitScale(42, "instructions")).toEqual({ factor: 1, symbol: "" });
		expect(unitScale(4200, "instructions")).toEqual({
			factor: 1e3,
			symbol: "x 1e3",
		});
	});

	// Kills taking the last parenthesis instead of the first.
	test("takes the first parenthesized symbol", () => {
		expect(unitScale(1, "a (b) (c)").symbol).toBe("b");
	});

	// Kills a scale that leaves the first tier for small or negative minimums.
	test("keeps the base unit for a minimum below one", () => {
		expect(unitScale(-5, "nanoseconds (ns)")).toEqual({
			factor: 1,
			symbol: "ns",
		});
		expect(unitScale(0.25, "bytes (B)")).toEqual({ factor: 1, symbol: "B" });
	});
});
