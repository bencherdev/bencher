import { describe, expect, test } from "vitest";
import { absoluteTime, duration, relativeTime, shortHash } from "./format";

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
