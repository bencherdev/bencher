// @vitest-environment happy-dom
import { describe, expect, test } from "vitest";
import { init_valid } from "./valid";
import { scale_factor, scale_units, scale_units_symbol } from "./scale";

const wasmReady = await init_valid();

describe("scale_factor", () => {
	test("returns 1 for non-numeric min", () => {
		// biome-ignore lint/suspicious/noExplicitAny: testing wrong type
		expect(scale_factor("not-a-number" as any, "ns")).toBe(1);
	});

	test("returns 1 for non-string units", () => {
		// biome-ignore lint/suspicious/noExplicitAny: testing wrong type
		expect(scale_factor(100, 42 as any)).toBe(1);
	});

	test.skipIf(!wasmReady)("returns a number for valid inputs", () => {
		const result = scale_factor(0.001, "ns");
		expect(typeof result).toBe("number");
	});
});

describe("scale_units", () => {
	test("returns 'Units' for non-numeric min", () => {
		// biome-ignore lint/suspicious/noExplicitAny: testing wrong type
		expect(scale_units("not-a-number" as any, "ns")).toBe("Units");
	});

	test("returns 'Units' for non-string units", () => {
		// biome-ignore lint/suspicious/noExplicitAny: testing wrong type
		expect(scale_units(100, 42 as any)).toBe("Units");
	});

	test.skipIf(!wasmReady)("returns a string for valid inputs", () => {
		const result = scale_units(1000, "ns");
		expect(typeof result).toBe("string");
	});
});

describe("scale_units_symbol", () => {
	test("returns empty string for non-numeric min", () => {
		// biome-ignore lint/suspicious/noExplicitAny: testing wrong type
		expect(scale_units_symbol("not-a-number" as any, "ns")).toBe("");
	});

	test("returns empty string for non-string units", () => {
		// biome-ignore lint/suspicious/noExplicitAny: testing wrong type
		expect(scale_units_symbol(100, 42 as any)).toBe("");
	});

	test.skipIf(!wasmReady)("returns a string for valid inputs", () => {
		const result = scale_units_symbol(1000, "ns");
		expect(typeof result).toBe("string");
	});
});
