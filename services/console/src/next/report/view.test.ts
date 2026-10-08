import { describe, expect, test } from "vitest";
import {
	parseReportView,
	type ReportView,
	reportViewSearch,
	windowDays,
} from "./view";

const parse = (search: string) => parseReportView(new URLSearchParams(search));

const DEFAULT: ReportView = {
	group: "benchmark",
	sort: "name",
	window: "4w",
	search: "",
	expanded: [],
};

describe("the report page's URL", () => {
	// Kills a default that differs from the API's, or one written into every link.
	test("is empty for the default view", () => {
		expect(parse("")).toEqual(DEFAULT);
		expect(reportViewSearch(DEFAULT)).toBe("");
	});

	// Kills a field dropped on the way out or misread on the way in.
	test.each<[string, ReportView]>([
		["group", { ...DEFAULT, group: "measure" }],
		["sort", { ...DEFAULT, sort: "delta" }],
		["preset window", { ...DEFAULT, window: "3m" }],
		["custom window", { ...DEFAULT, window: 10 }],
		["search", { ...DEFAULT, search: "simd=avx2 & p99" }],
		["expanded rows", { ...DEFAULT, expanded: ["1x3k9qz0ab", "x"] }],
		[
			"every field",
			{
				group: "measure",
				sort: "delta",
				window: "1w",
				search: "blake3",
				expanded: ["k"],
			},
		],
	])("round trips the %s", (_, view) => {
		expect(parse(reportViewSearch(view))).toEqual(view);
	});

	// Kills spelling a value differently from the API's query, so the page could not pass it through.
	test("spells group and sort as the API does", () => {
		expect(
			reportViewSearch({ ...DEFAULT, group: "measure", sort: "delta" }),
		).toBe("group=measure&sort=delta");
	});

	// Kills trusting a hand-edited link.
	test.each([
		"group=variant",
		"sort=worst",
		"window=5y",
		"window=0d",
		"window=-3d",
		"window=1.5d",
		"window=d",
		"window=toString",
	])("reads %s as the default", (search) => {
		expect(parse(search)).toEqual(DEFAULT);
	});

	// Kills a search of only spaces, which would hide every line.
	test("trims the search", () => {
		expect(parse("search=%20%20blake3%20").search).toBe("blake3");
		expect(reportViewSearch({ ...DEFAULT, search: "   " })).toBe("");
	});

	// Kills a row listed twice, which one collapse would leave open.
	test("lists each expanded row once", () => {
		expect(parse("expanded=a&expanded=b&expanded=a").expanded).toEqual([
			"a",
			"b",
		]);
	});
});

describe("windowDays", () => {
	// Kills a window of the wrong length.
	test.each([
		["1w", 7],
		["4w", 28],
		["3m", 92],
		[10, 10],
	] as const)("%s reaches %d days back from the report", (window, days) => {
		expect(windowDays(window)).toBe(days);
	});

	// Kills a custom window past the year the API reads back, which it refuses.
	test("reaches back at most 366 days", () => {
		expect(windowDays(99_999)).toBe(366);
	});
});
