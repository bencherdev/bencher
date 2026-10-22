import { describe, expect, test } from "vitest";
import {
	PROJECTS_PATH,
	alertPath,
	organizationOf,
	pageName,
	parseConsolePath,
	projectPath,
	reportPath,
	runPath,
	tabOf,
} from "./paths";

describe("projectPath", () => {
	// Kills a link built from anything but the one base path.
	test("puts every tab under the project's place in the new console", () => {
		expect(projectPath("hashbrown")).toBe(`${PROJECTS_PATH}/hashbrown/`);
		expect(projectPath("hashbrown", "alerts")).toBe(
			`${PROJECTS_PATH}/hashbrown/alerts`,
		);
	});
});

describe("organizationOf", () => {
	// Kills a project or account path taken for an organization's page, and an
	// organization read from the wrong segment.
	test("reads the organization an organization path is about", () => {
		expect(organizationOf("/console/organizations/pompeii-llc/settings")).toBe(
			"pompeii-llc",
		);
		expect(
			organizationOf("/console/projects/hashbrown/settings"),
		).toBeUndefined();
		expect(
			organizationOf("/console/users/muriel-bagge/settings"),
		).toBeUndefined();
	});
});

describe("parseConsolePath", () => {
	// Kills a slug read from the wrong segment, and paths outside the console accepted.
	test("reads the project and the rest of the path", () => {
		expect(parseConsolePath("/console/projects/hashbrown")).toEqual({
			slug: "hashbrown",
			rest: [],
		});
		expect(parseConsolePath("/console/projects/hashbrown/")).toEqual({
			slug: "hashbrown",
			rest: [],
		});
		expect(parseConsolePath("/console/projects/hashbrown/reports/abc")).toEqual(
			{ slug: "hashbrown", rest: ["reports", "abc"] },
		);
		expect(parseConsolePath("/perf/hashbrown/reports")).toBeUndefined();
		expect(parseConsolePath("/console/projects/")).toBeUndefined();
	});
});

describe("tabOf", () => {
	// Kills a tab row that marks the wrong tab, or none, for a deep path.
	test("maps a path to the tab it belongs to", () => {
		expect(tabOf([])).toBe("runs");
		expect(tabOf(["explore"])).toBe("explore");
		expect(tabOf(["runs"])).toBe("runs");
		expect(tabOf(["runs", "abc"])).toBe("runs");
		expect(tabOf(["reports", "abc"])).toBe("runs");
		expect(tabOf(["reports"])).toBeUndefined();
		expect(tabOf(["alerts"])).toBe("alerts");
		expect(tabOf(["thresholds", "abc"])).toBe("thresholds");
		expect(tabOf(["plots"])).toBe("plots");
		expect(tabOf(["settings"])).toBe("settings");
		expect(tabOf(["branches", "main"])).toBe("settings");
		expect(tabOf(["keys"])).toBe("settings");
		expect(tabOf(["connections"])).toBe("settings");
		expect(tabOf(["nope"])).toBeUndefined();
	});
});

describe("pageName", () => {
	// Kills a report or an alert opened on its list, a deeper path taken for
	// one, and a tab's path drawn by another tab's page.
	test("names the page that draws a project path", () => {
		expect(pageName([])).toBe("Runs");
		expect(pageName(["explore"])).toBe("Explore");
		expect(pageName(["runs"])).toBe("Runs");
		expect(pageName(["runs", "abc"])).toBe("Run");
		expect(pageName(["runs", "abc", "def"])).toBe("Runs");
		expect(pageName(["reports", "abc"])).toBe("Report");
		expect(pageName(["reports"])).toBe("NotFound");
		expect(pageName(["reports", "abc", "def"])).toBe("NotFound");
		expect(pageName(["thresholds", "abc"])).toBe("Threshold");
		expect(pageName(["thresholds", "abc", "def"])).toBe("Thresholds");
		expect(pageName(["alerts", "abc"])).toBe("Alert");
		expect(pageName(["alerts", "abc", "def"])).toBe("Alerts");
		expect(pageName(["metrics", "abc"])).toBe("Metric");
		expect(pageName(["metrics"])).toBe("NotFound");
		expect(pageName(["metrics", "abc", "def"])).toBe("NotFound");
		expect(pageName(["branches", "main"])).toBe("Dimensions");
		expect(pageName(["nope"])).toBe("NotFound");
	});

	// Kills a dimension list or page drawn by Settings, whose chunk would then
	// carry them, and Settings, Keys, or Connections drawn by another page.
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
		expect(pageName(["keys"])).toBe("Settings");
		expect(pageName(["connections"])).toBe("Settings");
	});
});

// Kills a report link outside the project's reports.
test("reportPath is the report under its project's reports", () => {
	expect(reportPath("hashbrown", "abc")).toBe(
		`${PROJECTS_PATH}/hashbrown/reports/abc`,
	);
});

// Kills an alert link outside the project's Alerts.
test("alertPath is the alert under its project's Alerts", () => {
	expect(alertPath("hashbrown", "abc")).toBe(
		`${PROJECTS_PATH}/hashbrown/alerts/abc`,
	);
});

// Kills a run link outside the project's Runs.
test("runPath is the run under its project's Runs", () => {
	expect(runPath("hashbrown", "abc")).toBe(
		`${PROJECTS_PATH}/hashbrown/runs/abc`,
	);
});
