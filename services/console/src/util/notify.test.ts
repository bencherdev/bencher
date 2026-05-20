// @vitest-environment happy-dom
import { describe, expect, test } from "vitest";
import {
	NotifyKind,
	isNotifyKind,
	isNotifyText,
	isNotifyTimeout,
} from "./notify";

describe("isNotifyKind", () => {
	test("returns true for OK", () => {
		expect(isNotifyKind(NotifyKind.OK)).toBe(true);
	});

	test("returns true for ALERT", () => {
		expect(isNotifyKind(NotifyKind.ALERT)).toBe(true);
	});

	test("returns true for ERROR", () => {
		expect(isNotifyKind(NotifyKind.ERROR)).toBe(true);
	});

	test("returns true for raw string values", () => {
		expect(isNotifyKind("ok")).toBe(true);
		expect(isNotifyKind("alert")).toBe(true);
		expect(isNotifyKind("error")).toBe(true);
	});

	test("returns false for invalid string", () => {
		expect(isNotifyKind("warning")).toBe(false);
		expect(isNotifyKind("")).toBe(false);
	});

	test("returns false for undefined", () => {
		expect(isNotifyKind(undefined)).toBe(false);
	});
});

describe("isNotifyText", () => {
	test("returns true for non-empty string", () => {
		expect(isNotifyText("hello")).toBe(true);
	});

	test("returns false for empty string", () => {
		expect(isNotifyText("")).toBe(false);
	});

	test("returns false for undefined", () => {
		expect(isNotifyText(undefined)).toBe(false);
	});
});

describe("isNotifyTimeout", () => {
	test("returns true for integer string", () => {
		expect(isNotifyTimeout("3000")).toBe(true);
	});

	test("returns true for zero", () => {
		expect(isNotifyTimeout("0")).toBe(true);
	});

	test("returns false for non-integer string", () => {
		expect(isNotifyTimeout("abc")).toBe(false);
	});

	test("returns false for empty string", () => {
		expect(isNotifyTimeout("")).toBe(false);
	});

	test("returns false for undefined", () => {
		expect(isNotifyTimeout(undefined)).toBe(false);
	});
});
