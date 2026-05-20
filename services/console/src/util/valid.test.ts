import { describe, expect, test } from "vitest";
import {
	validString,
	validOptionString,
	validateNumber,
	validU32,
	validNonZeroU32,
} from "./valid";

describe("validString", () => {
	test("passes input through validator after trimming", () => {
		const alwaysTrue = () => true;
		expect(validString("  hello  ", alwaysTrue)).toBe(true);
	});

	test("trims whitespace before validating", () => {
		const isHello = (s: string) => s === "hello";
		expect(validString("  hello  ", isHello)).toBe(true);
	});

	test("returns false when validator rejects", () => {
		const alwaysFalse = () => false;
		expect(validString("test", alwaysFalse)).toBe(false);
	});
});

describe("validOptionString", () => {
	test("returns false for undefined", () => {
		expect(validOptionString(undefined, () => true)).toBe(false);
	});

	test("returns false for null", () => {
		expect(validOptionString(null, () => true)).toBe(false);
	});

	test("returns false for empty string", () => {
		expect(validOptionString("", () => true)).toBe(false);
	});

	test("delegates to validator for non-empty string", () => {
		expect(validOptionString("test", () => true)).toBe(true);
		expect(validOptionString("test", () => false)).toBe(false);
	});
});

describe("validateNumber", () => {
	test("validates numeric string", () => {
		const isPositive = (n: number) => n > 0;
		expect(validateNumber("42", isPositive)).toBe(true);
	});

	test("returns false for empty string", () => {
		expect(validateNumber("", () => true)).toBe(false);
	});

	test("returns false when validator rejects", () => {
		const isPositive = (n: number) => n > 0;
		expect(validateNumber("-1", isPositive)).toBe(false);
	});

	test("trims whitespace", () => {
		const isPositive = (n: number) => n > 0;
		expect(validateNumber("  5  ", isPositive)).toBe(true);
	});
});

describe("validU32", () => {
	test("returns true for 0", () => {
		expect(validU32(0)).toBe(true);
	});

	test("returns true for max u32", () => {
		expect(validU32(4_294_967_295)).toBe(true);
	});

	test("returns true for string number", () => {
		expect(validU32("100")).toBe(true);
	});

	test("returns false for negative", () => {
		expect(validU32(-1)).toBe(false);
	});

	test("returns false for value exceeding u32 max", () => {
		expect(validU32(4_294_967_296)).toBe(false);
	});

	test("returns false for float", () => {
		expect(validU32(1.5)).toBe(false);
	});

	test("returns false for undefined", () => {
		expect(validU32(undefined)).toBe(false);
	});

	test("returns false for empty string", () => {
		expect(validU32("")).toBe(false);
	});
});

describe("validNonZeroU32", () => {
	test("returns true for positive integer", () => {
		expect(validNonZeroU32(1)).toBe(true);
	});

	test("returns false for zero", () => {
		expect(validNonZeroU32(0)).toBe(false);
	});

	test("returns false for negative", () => {
		expect(validNonZeroU32(-1)).toBe(false);
	});

	test("returns true for string number", () => {
		expect(validNonZeroU32("42")).toBe(true);
	});

	test("returns false for undefined", () => {
		expect(validNonZeroU32(undefined)).toBe(false);
	});
});
