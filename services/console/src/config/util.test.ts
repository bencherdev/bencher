import { describe, expect, test } from "vitest";
import { fmtDateTime, parentPath, toCapitalized } from "./util";

describe("parentPath", () => {
	test("returns parent of nested path", () => {
		expect(parentPath("/console/projects/my-project")).toBe(
			"/console/projects",
		);
	});

	test("returns empty string for root-level path", () => {
		expect(parentPath("/projects")).toBe("");
	});
});

describe("toCapitalized", () => {
	test("capitalizes first letter", () => {
		expect(toCapitalized("hello")).toBe("Hello");
	});

	test("handles single character", () => {
		expect(toCapitalized("a")).toBe("A");
	});

	test("handles already capitalized", () => {
		expect(toCapitalized("Hello")).toBe("Hello");
	});

	test("handles empty string", () => {
		expect(toCapitalized("")).toBe("");
	});
});

describe("fmtDateTime", () => {
	test("formats valid date-time string", () => {
		const result = fmtDateTime("2024-01-15T12:00:00Z");
		expect(result).toContain("2024");
		expect(result).toContain("January");
		expect(result).toContain("15");
	});
});
