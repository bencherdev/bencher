import { describe, expect, test } from "vitest";
import type { JsonReport } from "../../types/bencher";
import {
	absoluteTime,
	creator,
	duration,
	lines,
	relativeTime,
	shortHash,
} from "./format";

const SECOND = 1_000;
const MINUTE = 60 * SECOND;
const HOUR = 60 * MINUTE;
const DAY = 24 * HOUR;
const NOW = Date.parse("2026-09-14T00:00:00Z");

describe("relativeTime", () => {
	// Kills a unit boundary off by one step and a rounded rather than floored count.
	test.each([
		[0, "just now"],
		[59 * SECOND, "just now"],
		[MINUTE, "1m ago"],
		[59 * MINUTE + 59 * SECOND, "59m ago"],
		[HOUR, "1h ago"],
		[2 * HOUR + 59 * MINUTE, "2h ago"],
		[DAY - 1, "23h ago"],
		[DAY, "1d ago"],
		[13 * DAY + 23 * HOUR, "13d ago"],
		[14 * DAY, "2w ago"],
		[59 * DAY, "8w ago"],
		[60 * DAY, "1mo ago"],
		[364 * DAY, "11mo ago"],
		[365 * DAY, "1y ago"],
		[800 * DAY, "2y ago"],
	])("%i ms before now reads %s", (before, text) => {
		expect(relativeTime(NOW - before, NOW)).toBe(text);
	});

	// Kills a negative count for a report whose clock ran ahead of the reader's.
	test("a time after now is just now", () => {
		expect(relativeTime(NOW + 5 * MINUTE, NOW)).toBe("just now");
	});
});

// Kills a 12 hour clock, a dropped year, and a time zone other than the one asked for.
test("absoluteTime names the date, the year, and a 24 hour time", () => {
	const time = Date.parse("2026-09-13T21:46:00Z");
	expect(absoluteTime(time, "UTC")).toBe("Sep 13, 2026, 21:46");
	expect(absoluteTime(time, "Asia/Tokyo")).toBe("Sep 14, 2026, 06:46");
});

describe("duration", () => {
	// Kills unpadded seconds or minutes, and a unit boundary off by one step.
	test.each([
		[0, "0s"],
		[999, "0s"],
		[42 * SECOND, "42s"],
		[2 * MINUTE, "2m 00s"],
		[4 * MINUTE + 12 * SECOND, "4m 12s"],
		[59 * MINUTE + 59 * SECOND, "59m 59s"],
		[HOUR, "1h 00m"],
		[26 * HOUR + 5 * MINUTE + 30 * SECOND, "26h 05m"],
	])("%i ms reads %s", (ms, text) => {
		expect(duration(ms)).toBe(text);
	});

	// Kills a negative duration for a report that ends before it starts.
	test("an end before the start is no time", () => {
		expect(duration(-5 * SECOND)).toBe("0s");
	});
});

// Kills a hash shown in full, and a short hash of the wrong length.
test("shortHash keeps the first seven characters", () => {
	expect(shortHash("9c1f2e4000000000000000000000000000000000")).toBe("9c1f2e4");
	expect(shortHash(undefined)).toBeUndefined();
});

const report = (fields: Partial<JsonReport>) => fields as JsonReport;

describe("creator", () => {
	// Kills a key run named by nobody, or by the person who made the key.
	test("a run by a project key is the key's", () => {
		expect(
			creator(
				report({
					project_key: { uuid: "k", name: "GitHub Actions" },
				}),
			),
		).toEqual({ kind: "key", name: "GitHub Actions" });
	});

	test("a run by a person is theirs", () => {
		expect(
			creator(
				report({
					user: { uuid: "u", name: "Muriel Bagge", slug: "muriel-bagge" },
				}),
			),
		).toEqual({ kind: "user", name: "Muriel Bagge" });
	});

	// Kills a key run named for the person behind the key when both are known.
	test("a key is named over a person", () => {
		expect(
			creator(
				report({
					project_key: { uuid: "k", name: "GitHub Actions" },
					user: { uuid: "u", name: "Muriel Bagge", slug: "muriel-bagge" },
				}),
			),
		).toEqual({ kind: "key", name: "GitHub Actions" });
	});

	// Kills a crash on a report whose creator was deleted.
	test("a run by no one left is nobody's", () => {
		expect(creator(report({}))).toBeUndefined();
	});
});

describe("lines", () => {
	// Kills a count taken from one iteration only, or summed over every iteration.
	test("the most lines any iteration reported", () => {
		expect(
			lines(
				report({
					counts: {
						results: [
							{ benchmarks: 2, measures: 1, lines: 3 },
							{ benchmarks: 2, measures: 2, lines: 5 },
						],
						alerts: { total: 0, active: 0 },
					},
				}),
			),
		).toBe(5);
	});

	// Kills a count of NaN for an API that predates the line count.
	test("no count is zero", () => {
		expect(
			lines(
				report({
					counts: {
						results: [{ benchmarks: 2, measures: 1 }],
						alerts: { total: 0, active: 0 },
					},
				}),
			),
		).toBe(0);
		expect(lines(report({}))).toBe(0);
	});
});
