import { describe, expect, test } from "vitest";
import {
	addPath,
	createdSlugPath,
	createdUuidPath,
	echoPath,
	fmtDateTime,
	invitePath,
	parentPath,
	perfPath,
	resourcePath,
	toCapitalized,
	viewSlugPath,
	viewUuidPath,
} from "./util";

describe("echoPath", () => {
	test("returns pathname unchanged", () => {
		expect(echoPath("/console/projects")).toBe("/console/projects");
	});
});

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

describe("createdSlugPath", () => {
	test("replaces last segment with slug", () => {
		expect(createdSlugPath("/console/projects/add", { slug: "my-proj" })).toBe(
			"/console/projects/my-proj",
		);
	});
});

describe("createdUuidPath", () => {
	test("replaces last segment with uuid", () => {
		expect(
			createdUuidPath("/console/tokens/add", {
				uuid: "abc-123",
			}),
		).toBe("/console/tokens/abc-123");
	});
});

describe("addPath", () => {
	test("appends /add", () => {
		expect(addPath("/console/projects")).toBe("/console/projects/add");
	});
});

describe("invitePath", () => {
	test("appends /invite", () => {
		expect(invitePath("/console/members")).toBe("/console/members/invite");
	});
});

describe("viewSlugPath", () => {
	test("appends slug", () => {
		expect(viewSlugPath("/console/projects", { slug: "bench" })).toBe(
			"/console/projects/bench",
		);
	});
});

describe("viewUuidPath", () => {
	test("appends uuid", () => {
		expect(viewUuidPath("/console/tokens", { uuid: "abc-def" })).toBe(
			"/console/tokens/abc-def",
		);
	});
});

describe("perfPath", () => {
	test("returns console path when isConsole is true", () => {
		expect(perfPath(true, "my-project")).toBe(
			"/console/projects/my-project/perf",
		);
	});

	test("returns console path when isConsole is undefined", () => {
		expect(perfPath(undefined, "my-project")).toBe(
			"/console/projects/my-project/perf",
		);
	});

	test("returns public path when isConsole is false", () => {
		expect(perfPath(false, "my-project")).toBe("/perf/my-project");
	});
});

describe("resourcePath", () => {
	test("returns console path when isConsole is true", () => {
		expect(resourcePath(true)).toBe("/console/projects");
	});

	test("returns console path when isConsole is undefined", () => {
		expect(resourcePath(undefined)).toBe("/console/projects");
	});

	test("returns public path when isConsole is false", () => {
		expect(resourcePath(false)).toBe("/perf");
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
