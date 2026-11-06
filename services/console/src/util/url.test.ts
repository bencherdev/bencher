// @vitest-environment happy-dom
import { describe, expect, test } from "vitest";
import { createMatcher, extractSearchParams, mergeSearchString } from "./url";

describe("extractSearchParams", () => {
	test("extracts params from URL", () => {
		const url = new URL("http://example.com?foo=bar&baz=qux");
		expect(extractSearchParams(url)).toEqual({ foo: "bar", baz: "qux" });
	});

	test("returns empty object for URL without params", () => {
		const url = new URL("http://example.com");
		expect(extractSearchParams(url)).toEqual({});
	});

	test("handles encoded values", () => {
		const url = new URL("http://example.com?key=hello%20world");
		expect(extractSearchParams(url)).toEqual({ key: "hello world" });
	});
});

describe("mergeSearchString", () => {
	test("merges params into empty search", () => {
		expect(mergeSearchString("", { foo: "bar" })).toBe("?foo=bar");
	});

	test("adds to existing params", () => {
		const result = mergeSearchString("?a=1", { b: "2" });
		expect(result).toContain("a=1");
		expect(result).toContain("b=2");
	});

	test("overwrites existing param", () => {
		expect(mergeSearchString("?a=1", { a: "2" })).toBe("?a=2");
	});

	test("removes param when value is null", () => {
		expect(mergeSearchString("?a=1&b=2", { a: null })).toBe("?b=2");
	});

	test("removes param when value is empty string", () => {
		expect(mergeSearchString("?a=1&b=2", { a: "" })).toBe("?b=2");
	});

	test("removes param when value is undefined", () => {
		expect(mergeSearchString("?a=1&b=2", { a: undefined })).toBe("?b=2");
	});

	test("returns empty string when all params removed", () => {
		expect(mergeSearchString("?a=1", { a: null })).toBe("");
	});

	test("handles boolean values", () => {
		expect(mergeSearchString("", { flag: true })).toBe("?flag=true");
	});

	test("handles numeric values", () => {
		expect(mergeSearchString("", { page: 3 })).toBe("?page=3");
	});
});

describe("createMatcher", () => {
	test("matches exact static path", () => {
		const matcher = createMatcher("/console/projects");
		const result = matcher("/console/projects");
		expect(result).not.toBeNull();
		expect(result?.path).toBe("/console/projects");
		expect(result?.params).toEqual({});
	});

	test("returns null for non-matching path", () => {
		const matcher = createMatcher("/console/projects");
		expect(matcher("/console/users")).toBeNull();
	});

	test("extracts parameterized segments", () => {
		const matcher = createMatcher("/projects/:slug");
		const result = matcher("/projects/my-project");
		expect(result).not.toBeNull();
		expect(result?.params).toEqual({ slug: "my-project" });
	});

	test("extracts multiple params", () => {
		const matcher = createMatcher("/orgs/:org/projects/:project");
		const result = matcher("/orgs/acme/projects/bench");
		expect(result).not.toBeNull();
		expect(result?.params).toEqual({ org: "acme", project: "bench" });
	});

	test("returns null when path is too short", () => {
		const matcher = createMatcher("/a/b/c");
		expect(matcher("/a/b")).toBeNull();
	});

	test("returns null when path is too long without splat or partial", () => {
		const matcher = createMatcher("/a/b");
		expect(matcher("/a/b/c")).toBeNull();
	});

	test("matches with splat route", () => {
		const matcher = createMatcher("/files/*rest");
		const result = matcher("/files/docs/readme.md");
		expect(result).not.toBeNull();
		expect(result?.params).toEqual({ rest: "docs/readme.md" });
	});

	test("splat captures empty string when no extra segments", () => {
		const matcher = createMatcher("/files/*rest");
		const result = matcher("/files");
		expect(result).not.toBeNull();
		expect(result?.params).toEqual({ rest: "" });
	});

	test("matches longer path with partial flag", () => {
		const matcher = createMatcher("/console", true);
		const result = matcher("/console/projects/foo");
		expect(result).not.toBeNull();
		expect(result?.path).toBe("/console");
	});

	test("case-insensitive matching for static segments", () => {
		const matcher = createMatcher("/Console/Projects");
		const result = matcher("/console/projects");
		expect(result).not.toBeNull();
	});

	test("matches root path pattern", () => {
		const matcher = createMatcher("/");
		const result = matcher("/");
		expect(result).not.toBeNull();
		expect(result?.path).toBe("/");
	});
});
