import { describe, expect, test } from "vitest";
import {
	DEFAULT_SEARCH,
	DEFAULT_VIEW,
	alertsParams,
	decodeSearch,
	decodeView,
	encodeSearch,
	encodeView,
	historyDays,
	pickWindow,
	thresholdsParams,
	windowPhrase,
} from "./search";

const DAY = 86_400_000;
const NOW = Date.UTC(2026, 8, 14);
const BRANCH = "7b5d3f39-ec2c-4b46-9c55-1d0b6c2f5a10";
const MEASURE = "e9c1b0c4-6a5e-4f5f-8d1c-3b2a19f0a7d2";

const params = (query: string) => new URLSearchParams(query);

describe("the list's link", () => {
	// Kills a default written into the link, which would make a blank list's link carry a query.
	test("leaves out the defaults", () => {
		expect(encodeSearch(DEFAULT_SEARCH)).toBe("");
		expect(decodeSearch(params(""))).toEqual(DEFAULT_SEARCH);
	});

	// Kills a field lost between the link and the page, either way.
	test("round trips the status, the filters, and the window", () => {
		for (const search of [
			{
				...DEFAULT_SEARCH,
				archived: true,
				branch: BRANCH,
				measure: MEASURE,
				window: { kind: "rolling", days: 7 },
			},
			{
				...DEFAULT_SEARCH,
				testbed: BRANCH,
				window: { kind: "custom", start: NOW - 9 * DAY, end: NOW },
			},
			{ ...DEFAULT_SEARCH, window: { kind: "rolling", days: 92 } },
		] as const) {
			expect(decodeSearch(params(encodeSearch(search)))).toEqual(search);
		}
	});

	// Kills a hand edited value taken at its word: the API refuses a filter that is not a UUID.
	test("drops what does not parse", () => {
		expect(
			decodeSearch(
				params(
					"status=archive&branch=main&testbed=&window=2w&start_time=9&end_time=1",
				),
			),
		).toEqual(DEFAULT_SEARCH);
	});
});

describe("the list's request", () => {
	// Kills a filter, the archived flag, or a batch sent under another name or not at all.
	test("sends the filters, the status, and the batch", () => {
		expect(
			Object.fromEntries(
				thresholdsParams(
					{
						...DEFAULT_SEARCH,
						archived: true,
						branch: BRANCH,
						testbed: BRANCH,
						measure: MEASURE,
					},
					NOW,
					{ page: 3, perPage: 24 },
				),
			),
		).toEqual({
			archived: "true",
			branch: BRANCH,
			testbed: BRANCH,
			measure: MEASURE,
			start_time: String(NOW - 28 * DAY),
			page: "3",
			per_page: "24",
		});
	});

	// Kills Active sent as `archived=false`, which the API reads the same, and a custom range that loses an end.
	test("asks for active thresholds by leaving the flag out, and a range by both ends", () => {
		const sent = thresholdsParams(
			{
				...DEFAULT_SEARCH,
				window: { kind: "custom", start: NOW - DAY, end: NOW },
			},
			NOW,
			{ page: 1, perPage: 8 },
		);
		expect(sent.has("archived")).toBe(false);
		expect(sent.get("start_time")).toBe(String(NOW - DAY));
		expect(sent.get("end_time")).toBe(String(NOW));
	});
});

describe("a threshold's link", () => {
	// Kills a default other than All, as the board draws it, written into the link.
	test("leaves out the defaults, All for every alert", () => {
		expect(encodeView(DEFAULT_VIEW)).toBe("");
		expect(decodeView(params(""))).toEqual(DEFAULT_VIEW);
	});

	// Kills a status or window lost between the link and the page.
	test("round trips the status and the window", () => {
		for (const view of [
			{ status: "active", window: { kind: "rolling", days: 7 } },
			{
				status: "dismissed",
				window: { kind: "custom", start: NOW - DAY, end: NOW },
			},
		] as const) {
			expect(decodeView(params(encodeView(view)))).toEqual(view);
		}
		expect(decodeView(params("status=silenced"))).toEqual(DEFAULT_VIEW);
	});
});

describe("a threshold's alerts request", () => {
	// Kills the status left to the API's default, the threshold sent under another name, and each row's history not following the window.
	test("names the threshold, the status, the window, and each row's history", () => {
		expect(
			Object.fromEntries(
				alertsParams(BRANCH, DEFAULT_VIEW, NOW, { page: 2, perPage: 16 }),
			),
		).toEqual({
			thresholds: BRANCH,
			status: "all",
			start_time: String(NOW - 28 * DAY),
			window: "28",
			page: "2",
			per_page: "16",
		});
	});
});

describe("historyDays", () => {
	// Kills a history that ignores a custom range's length, or one past what the API reads.
	test("spans the window in whole days, from 1 to 366", () => {
		expect(historyDays({ kind: "rolling", days: 92 })).toBe(92);
		expect(
			historyDays({ kind: "custom", start: NOW - 10 * DAY, end: NOW - 1 }),
		).toBe(10);
		expect(historyDays({ kind: "custom", start: NOW, end: NOW })).toBe(1);
		expect(historyDays({ kind: "custom", start: 0, end: NOW })).toBe(366);
	});
});

describe("windowPhrase", () => {
	// Kills a phrase that reads as a sentence start in the middle of one, and
	// one week read as "1 week".
	test("reads inside a sentence", () => {
		expect(windowPhrase({ kind: "rolling", days: 28 }, NOW, "UTC")).toBe(
			"in the last 4 weeks",
		);
		expect(windowPhrase({ kind: "rolling", days: 7 }, NOW, "UTC")).toBe(
			"in the last week",
		);
		expect(
			windowPhrase(
				{ kind: "custom", start: Date.UTC(2026, 7, 18), end: NOW - 1 },
				NOW,
				"UTC",
			),
		).toBe("from Aug 18 to Sep 13");
	});
});

describe("pickWindow", () => {
	// Kills Custom resetting a range already chosen, a preset's days, and a first Custom that is not the last four weeks.
	test("keeps a custom range, else starts one at the last four weeks", () => {
		const range = { kind: "custom", start: NOW - 3 * DAY, end: NOW } as const;
		expect(pickWindow("custom", range, NOW)).toBe(range);
		expect(pickWindow("3m", range, NOW)).toEqual({ kind: "rolling", days: 92 });
		const fresh = pickWindow("custom", DEFAULT_SEARCH.window, NOW);
		expect(fresh?.kind).toBe("custom");
		expect(
			fresh?.kind === "custom" && Math.round((fresh.end - fresh.start) / DAY),
		).toBe(28);
	});
});
