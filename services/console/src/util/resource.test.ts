import { describe, expect, test } from "vitest";
import {
	BENCHER_TITLE,
	fmtNestedValue,
	fmtPageTitle,
	fmtValues,
} from "./resource";

describe("fmtValues", () => {
	test("returns value by key", () => {
		const data = { name: "test", count: 42 };
		expect(fmtValues(data, "name", undefined, " ")).toBe("test");
	});

	test("returns numeric value by key", () => {
		const data = { count: 42 };
		expect(fmtValues(data, "count", undefined, " ")).toBe(42);
	});

	test("returns undefined for undefined data", () => {
		expect(fmtValues(undefined, "name", undefined, " ")).toBeUndefined();
	});

	test("returns nested values joined by separator", () => {
		const data = { first: "John", last: "Doe" } as Record<string, string>;
		const keys = [["first"], ["last"]];
		expect(fmtValues(data, undefined, keys, " ")).toBe("John Doe");
	});

	test("returns 'Unknown Item' when no key or keys", () => {
		const data = { name: "test" };
		expect(fmtValues(data, undefined, undefined, " ")).toBe("Unknown Item");
	});
});

describe("fmtNestedValue", () => {
	test("returns value for simple key path", () => {
		const datum = { name: "test" };
		expect(fmtNestedValue(datum, ["name"])).toBe("test");
	});

	test("returns empty string for undefined datum", () => {
		expect(fmtNestedValue(undefined, ["name"])).toBe("");
	});

	test("returns empty string for undefined keys", () => {
		expect(fmtNestedValue({ name: "test" }, undefined)).toBe("");
	});
});

describe("fmtPageTitle", () => {
	test("formats title with Bencher suffix", () => {
		expect(fmtPageTitle("Projects")).toBe(`Projects | ${BENCHER_TITLE}`);
	});

	test("returns just Bencher title for undefined", () => {
		expect(fmtPageTitle(undefined)).toBe(BENCHER_TITLE);
	});

	test("returns just Bencher title for empty string", () => {
		expect(fmtPageTitle("")).toBe(BENCHER_TITLE);
	});
});
