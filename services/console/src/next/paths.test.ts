import { describe, expect, test } from "vitest";
import {
	NEXT_PROJECTS,
	classicHref,
	nextHref,
	pageName,
	parseNextPath,
	projectPath,
	reportPath,
	tabOf,
} from "./paths";

const place = (path: string) => new URL(path, "https://bencher.dev");

describe("projectPath", () => {
	// Kills a link built from anything but the one base path.
	test("puts every tab under the project's place in the new console", () => {
		expect(projectPath("hashbrown")).toBe(`${NEXT_PROJECTS}/hashbrown/`);
		expect(projectPath("hashbrown", "alerts")).toBe(
			`${NEXT_PROJECTS}/hashbrown/alerts`,
		);
	});
});

describe("parseNextPath", () => {
	// Kills a slug read from the wrong segment, and paths outside the console accepted.
	test("reads the project and the rest of the path", () => {
		expect(parseNextPath("/next/console/projects/hashbrown")).toEqual({
			slug: "hashbrown",
			rest: [],
		});
		expect(parseNextPath("/next/console/projects/hashbrown/")).toEqual({
			slug: "hashbrown",
			rest: [],
		});
		expect(
			parseNextPath("/next/console/projects/hashbrown/reports/abc"),
		).toEqual({ slug: "hashbrown", rest: ["reports", "abc"] });
		expect(
			parseNextPath("/console/projects/hashbrown/reports"),
		).toBeUndefined();
		expect(parseNextPath("/next/console/projects/")).toBeUndefined();
	});
});

describe("tabOf", () => {
	// Kills a tab row that marks the wrong tab, or none, for a deep path.
	test("maps a path to the tab it belongs to", () => {
		expect(tabOf([])).toBe("explore");
		expect(tabOf(["explore"])).toBe("explore");
		expect(tabOf(["reports", "abc"])).toBe("reports");
		expect(tabOf(["alerts"])).toBe("alerts");
		expect(tabOf(["thresholds", "abc"])).toBe("thresholds");
		expect(tabOf(["plots"])).toBe("plots");
		expect(tabOf(["settings"])).toBe("settings");
		expect(tabOf(["branches", "main"])).toBe("settings");
		expect(tabOf(["keys"])).toBe("settings");
		expect(tabOf(["nope"])).toBeUndefined();
	});
});

describe("classicHref", () => {
	// Kills a version gate that sends a reader somewhere other than the same
	// place, or carries a query the other console does not read.
	test("sends a new console path to the same place in the classic console", () => {
		expect(
			classicHref(place("/next/console/projects/hashbrown/reports?page=2#top")),
		).toBe("/console/projects/hashbrown/reports#top");
		expect(
			classicHref(place("/next/console/projects/hashbrown/thresholds/abc")),
		).toBe("/console/projects/hashbrown/thresholds/abc");
	});

	// Kills Explore sent to a classic page that does not exist.
	test("sends Explore and the project root to the classic perf page", () => {
		for (const pathname of [
			"/next/console/projects/hashbrown",
			"/next/console/projects/hashbrown/",
			"/next/console/projects/hashbrown/explore",
		]) {
			expect(classicHref(place(`${pathname}?x=1`))).toBe(
				"/console/projects/hashbrown/perf",
			);
		}
	});

	// Kills a redirect out of a page the new console does not own.
	// Kills a version 0 project sent from the new Keys page to a classic page
	// that does not exist.
	test("sends Settings' Keys to the classic keys list", () => {
		expect(
			classicHref(place("/next/console/projects/hashbrown/settings/keys")),
		).toBe("/console/projects/hashbrown/keys");
	});

	test("leaves paths outside the new console alone", () => {
		expect(classicHref(place("/console/projects/x"))).toBeUndefined();
	});
});

describe("nextHref", () => {
	// Kills a version gate that sends a reader somewhere other than the same
	// place, or carries the classic paging into the new console.
	test("sends a classic project path to the same place in the new console", () => {
		expect(
			nextHref(place("/console/projects/hashbrown/reports?page=2#top")),
		).toBe("/next/console/projects/hashbrown/reports#top");
		expect(nextHref(place("/console/projects/hashbrown/branches/main"))).toBe(
			"/next/console/projects/hashbrown/branches/main",
		);
	});

	// Kills the classic perf page sent to a new console page that does not exist.
	test("sends the classic perf page to Explore", () => {
		expect(nextHref(place("/console/projects/hashbrown/perf?branches=x"))).toBe(
			"/next/console/projects/hashbrown/explore",
		);
	});

	// Kills a reader sent to a creation form the new console does not have.
	// Kills a classic key page sent to a path the new console does not draw:
	// keys live in Settings, as one list.
	test("sends the classic key pages to Settings' Keys", () => {
		for (const rest of [
			"keys",
			"keys/add",
			"keys/0e5a0c32-8d3f-4b52-9a7e-3f8e0f1c2d4b",
		]) {
			expect(nextHref(place(`/console/projects/hashbrown/${rest}`))).toBe(
				`${NEXT_PROJECTS}/hashbrown/settings/keys`,
			);
		}
	});

	test("sends the classic add and edit forms to the list or the page they belong to", () => {
		expect(nextHref(place("/console/projects/hashbrown/thresholds/add"))).toBe(
			"/next/console/projects/hashbrown/thresholds",
		);
		expect(
			nextHref(place("/console/projects/hashbrown/thresholds/abc/edit")),
		).toBe("/next/console/projects/hashbrown/thresholds/abc");
	});

	// Kills a redirect out of a classic page that is not a project's.
	test("leaves paths outside a classic project alone", () => {
		expect(nextHref(place("/console/projects"))).toBeUndefined();
	});

	// Kills a reader sent from a classic page to "Page not found".
	test("leaves classic pages the new console has no page for alone", () => {
		expect(
			nextHref(place("/console/projects/hashbrown/metrics/abc")),
		).toBeUndefined();
		expect(
			nextHref(place("/console/projects/hashbrown/metrics")),
		).toBeUndefined();
		expect(nextHref(place("/console/projects/hashbrown/nope"))).toBeUndefined();
	});
});

describe("pageName", () => {
	// Kills a report opened on the Reports list, a deeper path taken for a
	// report, and a tab's path drawn by another tab's page.
	test("names the page that draws a project path", () => {
		expect(pageName([])).toBe("Explore");
		expect(pageName(["reports"])).toBe("Reports");
		expect(pageName(["reports", "abc"])).toBe("Report");
		expect(pageName(["reports", "abc", "def"])).toBe("Reports");
		expect(pageName(["thresholds", "abc"])).toBe("Threshold");
		expect(pageName(["thresholds", "abc", "def"])).toBe("Thresholds");
		expect(pageName(["branches", "main"])).toBe("Dimensions");
		expect(pageName(["nope"])).toBe("NotFound");
	});

	// Kills a dimension list or page drawn by Settings, whose chunk would then
	// carry them, and Settings or Keys drawn by the dimensions' page.
	test("the dimension lists and their pages are one page, beside Settings", () => {
		for (const dimension of [
			"branches",
			"testbeds",
			"benchmarks",
			"measures",
		]) {
			expect(pageName([dimension])).toBe("Dimensions");
			expect(pageName([dimension, "a-slug"])).toBe("Dimensions");
		}
		expect(pageName(["settings"])).toBe("Settings");
		expect(pageName(["settings", "keys"])).toBe("Settings");
		expect(pageName(["keys"])).toBe("Settings");
	});
});

// Kills a report link outside the project's Reports.
test("reportPath is the report under its project's Reports", () => {
	expect(reportPath("hashbrown", "abc")).toBe(
		`${NEXT_PROJECTS}/hashbrown/reports/abc`,
	);
});
