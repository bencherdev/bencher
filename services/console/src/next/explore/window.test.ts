import { describe, expect, test } from "vitest";
import {
	customRange,
	dateValue,
	pickWindow,
	rollingName,
	rollingParts,
	rollingSeconds,
	windowChoice,
} from "./window";

const DAY = 24 * 60 * 60;
const NOW = Date.UTC(2026, 8, 14);

describe("windowChoice", () => {
	// Kills a preset read at the wrong length, so its segment never shows checked.
	test("name the preset a window is, if any", () => {
		expect(windowChoice({ seconds: 7 * DAY })).toBe("1w");
		expect(windowChoice({ seconds: 28 * DAY, end: NOW })).toBe("4w");
		expect(windowChoice({ seconds: 92 * DAY })).toBe("3m");
		expect(windowChoice({ seconds: 90 * DAY })).toBeUndefined();
		expect(windowChoice({ start: 0, end: NOW })).toBe("custom");
	});
});

describe("pickWindow", () => {
	// Kills a pick that loses the window's end (a report's), or a custom range that starts nowhere near it.
	test("keep where the window ends", () => {
		expect(pickWindow({ seconds: 7 * DAY, end: NOW }, "3m", NOW + 1)).toEqual({
			seconds: 92 * DAY,
			end: NOW,
		});
		expect(pickWindow({ seconds: 7 * DAY }, "custom", NOW)).toEqual({
			start: NOW - 7 * DAY * 1000,
			end: NOW,
		});
		expect(pickWindow({ seconds: DAY, end: NOW }, "custom", NOW + 5)).toEqual({
			start: NOW - DAY * 1000,
			end: NOW,
		});
		const custom = { start: 5, end: 9 };
		expect(pickWindow(custom, "custom", NOW)).toBe(custom);
		expect(pickWindow(custom, "1w", NOW)).toEqual({ seconds: 7 * DAY, end: 9 });
	});
});

describe("customRange", () => {
	// Kills a range that drops its last day or reads a date in another time zone than the reader's.
	test("span whole local days from the first to the last", () => {
		const start = new Date(2026, 8, 1).getTime();
		const end = new Date(2026, 8, 14).getTime() - 1;
		expect(customRange("2026-09-01", "2026-09-13")).toEqual({ start, end });
		expect(dateValue(start)).toBe("2026-09-01");
		expect(dateValue(end)).toBe("2026-09-13");
		expect(customRange("2026-09-13", "2026-09-01")).toBeUndefined();
		expect(customRange("", "2026-09-01")).toBeUndefined();
	});
});

describe("rollingName", () => {
	// Kills naming a pin's window by a preset it is not, or in seconds.
	test("name a rolling window by its preset, else in days", () => {
		expect(rollingName(92 * DAY)).toBe("3m");
		expect(rollingName(7 * DAY)).toBe("1w");
		expect(rollingName(10 * DAY)).toBe("10 days");
		expect(rollingName(DAY)).toBe("1 day");
		expect(rollingName(3 * 60 * 60)).toBe("1 day");
	});

	// Kills naming whole weeks in days.
	test("name whole weeks in weeks", () => {
		expect(rollingName(42 * DAY)).toBe("6 weeks");
		expect(rollingName(14 * DAY)).toBe("2 weeks");
	});
});

describe("rollingSeconds", () => {
	// Kills a week of the wrong length, a count that is not whole, and a window
	// of nothing or past the longest the console reads.
	test("make a rolling window of days or weeks", () => {
		expect(rollingSeconds(6, "weeks")).toBe(42 * DAY);
		expect(rollingSeconds(10, "days")).toBe(10 * DAY);
		expect(rollingSeconds(2.6, "days")).toBe(3 * DAY);
		expect(rollingSeconds(0, "days")).toBe(DAY);
		expect(rollingSeconds(Number.NaN, "weeks")).toBe(7 * DAY);
		expect(rollingSeconds(400, "days")).toBe(366 * DAY);
		expect(rollingSeconds(60, "weeks")).toBe(366 * DAY);
	});
});

describe("rollingParts", () => {
	// Kills a pin opened in days when it saved whole weeks, or the reverse.
	test("read a rolling window back as weeks when whole, else days", () => {
		expect(rollingParts(42 * DAY)).toEqual({ count: 6, unit: "weeks" });
		expect(rollingParts(10 * DAY)).toEqual({ count: 10, unit: "days" });
		expect(rollingParts(3 * 60 * 60)).toEqual({ count: 1, unit: "days" });
	});
});
