import { describe, expect, test } from "vitest";
import { XAxis, YAxis } from "../types/bencher";
import {
	BencherResource,
	PerfTab,
	isPerfTab,
	isXAxis,
	isYAxis,
	resourcePlural,
	resourceSingular,
} from "./types";

describe("resourceSingular", () => {
	const cases: [BencherResource, string][] = [
		[BencherResource.ORGANIZATIONS, "organization"],
		[BencherResource.MEMBERS, "member"],
		[BencherResource.BILLING, "billing"],
		[BencherResource.PROJECTS, "project"],
		[BencherResource.REPORTS, "report"],
		[BencherResource.BRANCHES, "branch"],
		[BencherResource.TESTBEDS, "testbed"],
		[BencherResource.BENCHMARKS, "benchmark"],
		[BencherResource.MEASURES, "measure"],
		[BencherResource.METRICS, "metric"],
		[BencherResource.THRESHOLDS, "threshold"],
		[BencherResource.ALERTS, "alert"],
		[BencherResource.USERS, "user"],
		[BencherResource.USER_KEYS, "user key"],
		[BencherResource.TOKENS, "token"],
		[BencherResource.PROJECT_KEYS, "project key"],
		[BencherResource.HELP, "help"],
	];

	test.each(cases)("%s -> %s", (resource, expected) => {
		expect(resourceSingular(resource)).toBe(expected);
	});
});

describe("resourcePlural", () => {
	const cases: [BencherResource, string][] = [
		[BencherResource.ORGANIZATIONS, "organizations"],
		[BencherResource.MEMBERS, "members"],
		[BencherResource.BILLING, "billing"],
		[BencherResource.PROJECTS, "projects"],
		[BencherResource.REPORTS, "reports"],
		[BencherResource.BRANCHES, "branches"],
		[BencherResource.TESTBEDS, "testbeds"],
		[BencherResource.BENCHMARKS, "benchmarks"],
		[BencherResource.MEASURES, "measures"],
		[BencherResource.METRICS, "metrics"],
		[BencherResource.THRESHOLDS, "thresholds"],
		[BencherResource.ALERTS, "alerts"],
		[BencherResource.USERS, "users"],
		[BencherResource.USER_KEYS, "user keys"],
		[BencherResource.TOKENS, "tokens"],
		[BencherResource.PROJECT_KEYS, "project keys"],
		[BencherResource.HELP, "help"],
	];

	test.each(cases)("%s -> %s", (resource, expected) => {
		expect(resourcePlural(resource)).toBe(expected);
	});
});

describe("isPerfTab", () => {
	test("returns true for each PerfTab value", () => {
		expect(isPerfTab(PerfTab.REPORTS)).toBe(true);
		expect(isPerfTab(PerfTab.BRANCHES)).toBe(true);
		expect(isPerfTab(PerfTab.TESTBEDS)).toBe(true);
		expect(isPerfTab(PerfTab.BENCHMARKS)).toBe(true);
		expect(isPerfTab(PerfTab.PLOTS)).toBe(true);
	});

	test("returns true for raw string values", () => {
		expect(isPerfTab("reports")).toBe(true);
		expect(isPerfTab("branches")).toBe(true);
	});

	test("returns false for invalid string", () => {
		expect(isPerfTab("invalid")).toBe(false);
		expect(isPerfTab("")).toBe(false);
	});

	test("returns false for undefined", () => {
		expect(isPerfTab(undefined)).toBe(false);
	});
});

describe("isXAxis", () => {
	test("returns true for DateTime", () => {
		expect(isXAxis(XAxis.DateTime)).toBe(true);
		expect(isXAxis("date_time")).toBe(true);
	});

	test("returns true for Version", () => {
		expect(isXAxis(XAxis.Version)).toBe(true);
		expect(isXAxis("version")).toBe(true);
	});

	test("returns false for invalid string", () => {
		expect(isXAxis("invalid")).toBe(false);
	});

	test("returns false for undefined", () => {
		expect(isXAxis(undefined)).toBe(false);
	});
});

describe("isYAxis", () => {
	test("returns true for Auto", () => {
		expect(isYAxis(YAxis.Auto)).toBe(true);
		expect(isYAxis("auto")).toBe(true);
	});

	test("returns true for Linear", () => {
		expect(isYAxis(YAxis.Linear)).toBe(true);
		expect(isYAxis("linear")).toBe(true);
	});

	test("returns true for Log", () => {
		expect(isYAxis(YAxis.Log)).toBe(true);
		expect(isYAxis("log")).toBe(true);
	});

	test("returns false for invalid string", () => {
		expect(isYAxis("invalid")).toBe(false);
	});

	test("returns false for undefined", () => {
		expect(isYAxis(undefined)).toBe(false);
	});
});
