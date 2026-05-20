// @vitest-environment happy-dom
import { describe, expect, test } from "vitest";
import {
	NOTIFY_KIND_PARAM,
	NOTIFY_TEXT_PARAM,
	NotifyKind,
	forwardParams,
	notifyPath,
} from "./notify";

describe("forwardParams", () => {
	test("returns pathname when both params are null", () => {
		expect(forwardParams("/console", null, null)).toBe("/console");
	});

	test("returns pathname when both params are empty", () => {
		expect(forwardParams("/console", [], [])).toBe("/console");
	});

	test("sets params from setParams", () => {
		const result = forwardParams("/console", null, [["foo", "bar"]]);
		expect(result).toBe("/console?foo=bar");
	});

	test("sets multiple params", () => {
		const result = forwardParams("/path", null, [
			["a", "1"],
			["b", "2"],
		]);
		expect(result).toContain("a=1");
		expect(result).toContain("b=2");
	});
});

describe("notifyPath", () => {
	test("builds path with notify params", () => {
		const result = notifyPath(
			NotifyKind.OK,
			"Success!",
			"/console/projects",
			null,
			null,
		);
		expect(result).toContain(`${NOTIFY_KIND_PARAM}=ok`);
		expect(result).toContain(`${NOTIFY_TEXT_PARAM}=Success`);
		expect(result.startsWith("/console/projects?")).toBe(true);
	});

	test("includes additional setParams", () => {
		const result = notifyPath(NotifyKind.ERROR, "Failed", "/console", null, [
			["extra", "value"],
		]);
		expect(result).toContain("extra=value");
		expect(result).toContain(`${NOTIFY_KIND_PARAM}=error`);
	});
});
