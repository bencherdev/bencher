import { describe, expect, test } from "vitest";
import { Adapter } from "../../types/bencher";
import {
	ADAPTERS,
	DEFAULT_SEARCH,
	type ReportsSearch,
	apiParams,
	customWindow,
	dateValue,
	decodeSearch,
	encodeSearch,
	filterCount,
	nothingFound,
	windowPhrase,
} from "./search";

const DAY = 24 * 60 * 60 * 1_000;
const NOW = Date.parse("2026-09-14T00:00:00Z");

const decode = (query: string) => decodeSearch(new URLSearchParams(query));

describe("decodeSearch", () => {
	// Kills a blank link that is not the four week default.
	test("a blank link is every report of the last four weeks", () => {
		expect(decode("")).toEqual(DEFAULT_SEARCH);
		expect(DEFAULT_SEARCH).toEqual({
			window: { kind: "rolling", days: 28 },
			alerts: false,
		});
	});

	// Kills a filter dropped on the way in, or kept when blank.
	test("filters are read by name and blanks are ignored", () => {
		expect(
			decode("branch=main&testbed=macos-latest&adapter=magic&alerts=active"),
		).toEqual({
			window: { kind: "rolling", days: 28 },
			branch: "main",
			testbed: "macos-latest",
			adapter: "magic",
			alerts: true,
		});
		expect(decode("branch=&testbed=%20&alerts=any")).toEqual(DEFAULT_SEARCH);
	});

	// Kills an adapter the list never names (a language adapter is listed as magic).
	test("an adapter the list does not name is ignored", () => {
		expect(decode("adapter=rust")).toEqual(DEFAULT_SEARCH);
		expect(decode("adapter=nonsense")).toEqual(DEFAULT_SEARCH);
		expect(decode("adapter=rust_criterion").adapter).toBe("rust_criterion");
	});

	// Kills a window read only from the presets, and a hand-edited window that breaks the page.
	test.each([
		["window=1w", { kind: "rolling", days: 7 }],
		["window=92d", { kind: "rolling", days: 92 }],
		["window=10d", { kind: "rolling", days: 10 }],
		["window=3w", { kind: "rolling", days: 21 }],
		["window=all", { kind: "all" }],
		["window=0d", { kind: "rolling", days: 28 }],
		["window=-1w", { kind: "rolling", days: 28 }],
		["window=99999d", { kind: "rolling", days: 28 }],
		["window=1y", { kind: "rolling", days: 28 }],
	])("%s is %o", (query, window) => {
		expect(decode(query).window).toEqual(window);
	});

	// Kills a custom range that needs a window to say so, a range read backwards,
	// and half a range taken as a whole one.
	test("a start and an end are a custom range, whatever the window says", () => {
		const start = Date.parse("2026-08-02T00:00:00Z");
		const end = Date.parse("2026-09-13T23:59:59Z");
		expect(
			decode(`start_time=${start}&end_time=${end}&window=1w`).window,
		).toEqual({ kind: "custom", start, end });
		expect(decode(`start_time=${end}&end_time=${start}`).window).toEqual(
			DEFAULT_SEARCH.window,
		);
		expect(decode(`start_time=${start}`).window).toEqual(DEFAULT_SEARCH.window);
		expect(decode("start_time=x&end_time=y").window).toEqual(
			DEFAULT_SEARCH.window,
		);
	});
});

describe("encodeSearch", () => {
	// Kills a default written into the link, which would make a blank Reports link two links.
	test("the default search is no query at all", () => {
		expect(encodeSearch(DEFAULT_SEARCH)).toBe("");
	});

	// Kills a field dropped on the way out, and an order that changes the link.
	test("every filter and the window round trip in a fixed order", () => {
		const search: ReportsSearch = {
			window: { kind: "rolling", days: 7 },
			branch: "412/merge",
			testbed: "macos-latest",
			adapter: "json",
			alerts: true,
		};
		const query = encodeSearch(search);
		expect(query).toBe(
			"branch=412%2Fmerge&testbed=macos-latest&adapter=json&alerts=active&window=1w",
		);
		expect(decode(query)).toEqual(search);
	});

	// Kills a window written in a unit that does not read back.
	test.each([
		[{ kind: "rolling", days: 7 } as const, "window=1w"],
		[{ kind: "rolling", days: 92 } as const, "window=92d"],
		[{ kind: "rolling", days: 21 } as const, "window=3w"],
		[{ kind: "all" } as const, "window=all"],
		[{ kind: "custom", start: 1, end: 2 } as const, "start_time=1&end_time=2"],
	])("%o is %s", (window, query) => {
		expect(encodeSearch({ window, alerts: false })).toBe(query);
		expect(decode(query).window).toEqual(window);
	});
});

describe("apiParams", () => {
	const params = (search: ReportsSearch) =>
		Object.fromEntries(apiParams(search, NOW, { page: 2, perPage: 23 }));

	// Kills a rolling window measured from anything but now, a window sent as an
	// end time, and paging that is not the batch asked for.
	test("a rolling window starts that many days before now", () => {
		expect(params(DEFAULT_SEARCH)).toEqual({
			start_time: String(NOW - 28 * DAY),
			page: "2",
			per_page: "23",
		});
	});

	// Kills a filter that never reaches the API, and active alerts sent as anything but true.
	test("filters reach the API by its names", () => {
		expect(
			params({
				window: { kind: "all" },
				branch: "main",
				testbed: "macos-latest",
				adapter: "magic",
				alerts: true,
			}),
		).toEqual({
			branch: "main",
			testbed: "macos-latest",
			adapter: "magic",
			active_alerts: "true",
			page: "2",
			per_page: "23",
		});
	});

	// Kills a custom range that loses either end.
	test("a custom range sends both ends", () => {
		expect(
			params({ window: { kind: "custom", start: 5, end: 9 }, alerts: false }),
		).toEqual({ start_time: "5", end_time: "9", page: "2", per_page: "23" });
	});
});

// Kills the window counted as a filter, and a filter left out of the count.
test("filterCount counts the filters, not the window", () => {
	expect(filterCount(DEFAULT_SEARCH)).toBe(0);
	expect(
		filterCount({
			window: { kind: "all" },
			branch: "main",
			testbed: "t",
			adapter: "json",
			alerts: true,
		}),
	).toBe(4);
	expect(filterCount({ ...DEFAULT_SEARCH, alerts: true })).toBe(1);
});

describe("windowPhrase", () => {
	// Kills a preset named by its days, and a custom range or all time named as a preset.
	test.each([
		[{ kind: "rolling", days: 7 } as const, "In the last week"],
		[{ kind: "rolling", days: 28 } as const, "In the last 4 weeks"],
		[{ kind: "rolling", days: 92 } as const, "In the last 3 months"],
		[{ kind: "rolling", days: 1 } as const, "In the last day"],
		[{ kind: "rolling", days: 10 } as const, "In the last 10 days"],
		[{ kind: "rolling", days: 21 } as const, "In the last 3 weeks"],
		[{ kind: "all" } as const, "All time"],
		[
			{
				kind: "custom",
				start: Date.parse("2026-08-02T00:00:00Z"),
				end: Date.parse("2026-09-13T23:59:59Z"),
			} as const,
			"From Aug 2 to Sep 13",
		],
		[
			{
				kind: "custom",
				start: Date.parse("2025-12-30T00:00:00Z"),
				end: Date.parse("2026-01-02T23:59:59Z"),
			} as const,
			"From Dec 30, 2025 to Jan 2, 2026",
		],
	])("%o reads %s", (window, phrase) => {
		expect(windowPhrase(window, NOW, "UTC")).toBe(phrase);
	});
});

describe("nothingFound", () => {
	// Kills a zero state that hides a filter, names a filter that is not set, or
	// offers a wider window when there is none.
	test.each([
		[
			{
				window: { kind: "rolling", days: 7 },
				branch: "feature-simd",
				adapter: "magic",
				alerts: false,
			} as ReportsSearch,
			"Nothing on feature-simd with the magic adapter in the last week. Widen the window or clear the filters.",
		],
		[
			{
				window: { kind: "all" },
				branch: "main",
				testbed: "macos-latest",
				alerts: true,
			} as ReportsSearch,
			"Nothing on main and macos-latest with active alerts. Clear the filters.",
		],
		[
			{
				window: { kind: "rolling", days: 28 },
				adapter: "json",
				alerts: true,
			} as ReportsSearch,
			"Nothing with the json adapter and active alerts in the last 4 weeks. Widen the window or clear the filters.",
		],
		[DEFAULT_SEARCH, "Nothing in the last 4 weeks. Widen the window."],
	])("%o", (search, text) => {
		expect(nothingFound(search, NOW, "UTC")).toBe(text);
	});

	// Kills a zero state that names a branch or a testbed by its slug when its name is known.
	test("names the branch and the testbed by name", () => {
		expect(
			nothingFound(
				{
					window: { kind: "all" },
					branch: "412-merge",
					testbed: "linux-x86-64",
					alerts: false,
				},
				NOW,
				"UTC",
				{ branch: "412/merge", testbed: "Linux x86 64" },
			),
		).toBe("Nothing on 412/merge and Linux x86 64. Clear the filters.");
	});
});

// Kills a custom range that starts after midnight, ends before the last moment
// of its day, or shows a date other than the one picked.
test("customWindow covers whole local days and dateValue reads them back", () => {
	const window = customWindow("2026-08-02", "2026-09-13");
	expect(window).toEqual({
		kind: "custom",
		start: new Date(2026, 7, 2).getTime(),
		end: new Date(2026, 8, 14).getTime() - 1,
	});
	expect(dateValue(window?.kind === "custom" ? window.start : 0)).toBe(
		"2026-08-02",
	);
	expect(dateValue(window?.kind === "custom" ? window.end : 0)).toBe(
		"2026-09-13",
	);
	expect(customWindow("2026-09-13", "2026-08-02")).toBeUndefined();
	expect(customWindow("", "2026-08-02")).toBeUndefined();
});

// Kills an adapter the API lists that the filter cannot offer or read from a
// link, and a name the API does not know.
test("ADAPTERS is every adapter the API lists, a language adapter aside", () => {
	const languages = [
		"rust",
		"cpp",
		"go",
		"java",
		"c_sharp",
		"js",
		"python",
		"ruby",
		"shell",
		"dart",
	];
	expect([...ADAPTERS].sort()).toEqual(
		Object.values(Adapter)
			.filter((adapter) => !languages.includes(adapter))
			.sort(),
	);
});
