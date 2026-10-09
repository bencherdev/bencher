import { describe, expect, test } from "vitest";
import { decodeQuery } from "../query/query";
import { exploreSearchOf } from "./explore";
import { alertLinesOf } from "./rows";
import { alertsFixture } from "./testing";

const DAY = 86_400_000;
const NOW = Date.parse("2026-09-14T00:00:00Z");
const lines = alertLinesOf(
	alertsFixture([{ report: 0, alerts: [{ alert: 1 }] }]),
);

describe("exploreSearchOf", () => {
	// Kills Explore opened over a fixed window rather than the page's.
	test.each([
		[{ kind: "rolling", days: 7 } as const, { seconds: 7 * 86_400 }],
		[{ kind: "all" } as const, { seconds: 92 * 86_400 }],
		[
			{ kind: "custom", start: NOW - 3 * DAY, end: NOW } as const,
			{ start: NOW - 3 * DAY, end: NOW },
		],
	])("over %o, Explore opens over %o", (window, expected) => {
		expect(decodeQuery(exploreSearchOf(lines, window)).window).toEqual(
			expected,
		);
	});
});
