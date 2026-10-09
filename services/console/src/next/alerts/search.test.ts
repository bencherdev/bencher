import { describe, expect, test } from "vitest";
import {
	type AlertsSearch,
	DEFAULT_SEARCH,
	alertsParams,
	decodeSearch,
	dismissAllFilter,
	encodeSearch,
	filterCount,
	historyDays,
	windowBounds,
	withFilter,
} from "./search";

const DAY = 24 * 60 * 60 * 1_000;
const NOW = Date.parse("2026-09-14T00:00:00Z");
const BRANCH = "6f3c1a2e-0000-4000-8000-000000000001";
const TESTBED = "6f3c1a2e-0000-4000-8000-000000000002";
const MEASURE = "6f3c1a2e-0000-4000-8000-000000000003";

const decode = (query: string) => decodeSearch(new URLSearchParams(query));

describe("decodeSearch", () => {
	// Kills a blank link that is not the active alerts of the last four weeks.
	test("a blank link is the active alerts of the last four weeks", () => {
		expect(decode("")).toEqual(DEFAULT_SEARCH);
		expect(DEFAULT_SEARCH).toEqual({
			status: "active",
			window: { kind: "rolling", days: 28 },
		});
	});

	// Kills a status read from anything but its three names.
	test.each([
		["status=dismissed", "dismissed"],
		["status=all", "all"],
		["status=active", "active"],
		["status=silenced", "active"],
		["status=", "active"],
	])("%s is %s", (query, status) => {
		expect(decode(query).status).toBe(status);
	});

	// Kills a filter that takes a slug or a typo, which the API refuses.
	test("filters are UUIDs, and anything else is ignored", () => {
		expect(
			decode(`branch=${BRANCH}&testbed=${TESTBED}&measure=${MEASURE}`),
		).toEqual({
			...DEFAULT_SEARCH,
			branch: BRANCH,
			testbed: TESTBED,
			measure: MEASURE,
		});
		expect(decode("branch=main&testbed=&measure=latency")).toEqual(
			DEFAULT_SEARCH,
		);
	});

	// Kills a window that reads only the presets or loses a custom range.
	test.each([
		["window=1w", { kind: "rolling", days: 7 }],
		["window=92d", { kind: "rolling", days: 92 }],
		["window=13w", { kind: "rolling", days: 91 }],
		["window=all", { kind: "all" }],
		[
			`start_time=${NOW - 3 * DAY}&end_time=${NOW}`,
			{ kind: "custom", start: NOW - 3 * DAY, end: NOW },
		],
	])("%s", (query, window) => {
		expect(decode(query).window).toEqual(window);
	});
});

describe("encodeSearch", () => {
	// Kills a link that spells out the defaults, or reorders its keys between edits.
	test("defaults are left out and every field rides in a fixed order", () => {
		expect(encodeSearch(DEFAULT_SEARCH)).toBe("");
		expect(
			encodeSearch({
				status: "all",
				measure: MEASURE,
				testbed: TESTBED,
				branch: BRANCH,
				window: { kind: "all" },
			}),
		).toBe(
			`status=all&branch=${BRANCH}&testbed=${TESTBED}&measure=${MEASURE}&window=all`,
		);
	});

	// Kills a field lost between the page and its link.
	test.each<AlertsSearch>([
		{ status: "dismissed", window: { kind: "rolling", days: 7 } },
		{ status: "active", branch: BRANCH, window: { kind: "all" } },
		{
			status: "all",
			testbed: TESTBED,
			measure: MEASURE,
			window: { kind: "custom", start: NOW - DAY, end: NOW },
		},
	])("%o survives a round trip", (search) => {
		expect(decode(encodeSearch(search))).toEqual(search);
	});
});

describe("filters", () => {
	// Kills a cleared filter left behind as undefined, and a count that counts the status or window.
	test("set, clear, and count", () => {
		const search = withFilter(DEFAULT_SEARCH, "branch", BRANCH);
		expect(search).toEqual({ ...DEFAULT_SEARCH, branch: BRANCH });
		expect(filterCount(search)).toBe(1);
		expect(
			filterCount({
				...withFilter(search, "measure", MEASURE),
				status: "all",
				window: { kind: "all" },
			}),
		).toBe(2);
		const cleared = withFilter(search, "branch", undefined);
		expect(cleared).toEqual(DEFAULT_SEARCH);
		expect("branch" in cleared).toBe(false);
	});
});

describe("the list and Dismiss all", () => {
	// Kills a rolling window measured from another moment, an all-time window
	// that still sends a start, and a custom range that drops its end.
	test.each<[AlertsSearch["window"], ReturnType<typeof windowBounds>]>([
		[{ kind: "rolling", days: 28 }, { start_time: NOW - 28 * DAY }],
		[{ kind: "all" }, {}],
		[
			{ kind: "custom", start: NOW - 3 * DAY, end: NOW },
			{ start_time: NOW - 3 * DAY, end_time: NOW },
		],
	])("the window %o binds %o", (window, bounds) => {
		expect(windowBounds(window, NOW)).toEqual(bounds);
	});

	// Kills a list that drops a filter or the status, or pages by page number,
	// which skips the alerts a dismissal in place made room for.
	test("one batch's query names the status, the filters, the window, and where it starts", () => {
		const search: AlertsSearch = {
			status: "dismissed",
			branch: BRANCH,
			measure: MEASURE,
			window: { kind: "rolling", days: 7 },
		};
		const params = alertsParams(search, windowBounds(search.window, NOW), {
			offset: 61,
			perPage: 32,
		});
		expect(Object.fromEntries(params)).toEqual({
			status: "dismissed",
			branches: BRANCH,
			measures: MEASURE,
			start_time: String(NOW - 7 * DAY),
			window: "7",
			offset: "61",
			per_page: "32",
		});
	});

	// Kills a Dismiss all that changes alerts other than the ones the list
	// counted: a filter or a bound dropped or read again, whatever the status
	// on screen.
	test.each<AlertsSearch>([
		DEFAULT_SEARCH,
		{ status: "all", branch: BRANCH, window: { kind: "all" } },
		{
			status: "dismissed",
			testbed: TESTBED,
			measure: MEASURE,
			window: { kind: "custom", start: NOW - 3 * DAY, end: NOW },
		},
	])("Dismiss all selects exactly what %o counts as active", (search) => {
		const bounds = windowBounds(search.window, NOW);
		const listed = alertsParams({ ...search, status: "active" }, bounds, {
			offset: 0,
			perPage: 32,
		});
		const filter = dismissAllFilter(search, bounds, NOW);
		expect(filter.status).toBe("active");
		expect(filter.branches?.join(",")).toBe(
			listed.get("branches") ?? undefined,
		);
		expect(filter.testbeds?.join(",")).toBe(
			listed.get("testbeds") ?? undefined,
		);
		expect(filter.measures?.join(",")).toBe(
			listed.get("measures") ?? undefined,
		);
		expect(filter.start_time).toBe(
			listed.has("start_time") ? Number(listed.get("start_time")) : undefined,
		);
		expect(filter.end_time).toBe(NOW);
		expect(Object.keys(filter).sort()).toEqual(
			[
				"status",
				...(listed.has("branches") ? ["branches"] : []),
				...(listed.has("testbeds") ? ["testbeds"] : []),
				...(listed.has("measures") ? ["measures"] : []),
				...(listed.has("start_time") ? ["start_time"] : []),
				"end_time",
			].sort(),
		);
	});

	// Kills a Dismiss all that changes an alert raised after the list was read,
	// or one that reaches past the end of a custom window.
	test.each<[AlertsSearch["window"], number]>([
		[{ kind: "rolling", days: 28 }, NOW - DAY],
		[{ kind: "all" }, NOW - DAY],
		[{ kind: "custom", start: NOW - 3 * DAY, end: NOW }, NOW - DAY],
		[
			{ kind: "custom", start: NOW - 3 * DAY, end: NOW - 2 * DAY },
			NOW - 2 * DAY,
		],
	])("Dismiss all over %o ends at %i", (window, end) => {
		const filter = dismissAllFilter(
			{ ...DEFAULT_SEARCH, window },
			windowBounds(window, NOW),
			NOW - DAY,
		);
		expect(filter.end_time).toBe(end);
	});

	// Kills a row history longer than the API reads, shorter than a day, or
	// unbounded for all time.
	test.each<[AlertsSearch["window"], number]>([
		[{ kind: "rolling", days: 7 }, 7],
		[{ kind: "rolling", days: 3_650 }, 366],
		[{ kind: "custom", start: NOW - 3 * DAY, end: NOW }, 4],
		[{ kind: "custom", start: NOW, end: NOW }, 1],
		[{ kind: "all" }, 92],
	])("%o draws %i days of history", (window, days) => {
		expect(historyDays(window)).toBe(days);
	});
});
