import { describe, expect, test } from "vitest";
import { pageWindowSearch, plotsKey, tileBatch } from "./queries";

const DAY = 24 * 60 * 60 * 1000;
const NOW = Date.UTC(2026, 8, 14);

describe("pageWindowSearch", () => {
	// Kills a page window that leaves a plot at its own, a preset of the wrong
	// length or not ending now, a custom range that loses its end, and own
	// windows that end at the server's clock rather than the reader's.
	test("draw every plot over the page's window, else each over its own", () => {
		expect(pageWindowSearch(undefined, NOW)).toEqual({ end_time: String(NOW) });
		expect(pageWindowSearch("1w", NOW)).toEqual({
			start_time: String(NOW - 7 * DAY),
			end_time: String(NOW),
		});
		expect(pageWindowSearch("4w", NOW).start_time).toBe(String(NOW - 28 * DAY));
		expect(pageWindowSearch("3m", NOW).start_time).toBe(String(NOW - 92 * DAY));
		expect(pageWindowSearch({ start: 5, end: 9 }, NOW)).toEqual({
			start_time: "5",
			end_time: "9",
		});
	});
});

describe("plotsKey", () => {
	// Kills a key that holds the time a preset was asked at, so a revisit never
	// draws from the cache, one page window's tiles drawn for another, or a
	// batch drawn for a batch of another size on a screen of another size.
	test("name a batch by its page window, never the clock", () => {
		const batch = { page: 1, perPage: 8 };
		expect(plotsKey("p", "1w", batch)).toEqual(plotsKey("p", "1w", batch));
		const keys = [
			plotsKey("p", undefined, batch),
			plotsKey("p", "1w", batch),
			plotsKey("p", "4w", batch),
			plotsKey("p", { start: 5, end: 9 }, batch),
			plotsKey("p", { start: 5, end: 10 }, batch),
			plotsKey("p", undefined, { page: 2, perPage: 8 }),
			plotsKey("p", undefined, { page: 1, perPage: 6 }),
		].map((key) => JSON.stringify(key));
		expect(new Set(keys).size).toBe(keys.length);
	});
});

describe("tileBatch", () => {
	// Kills a batch that leaves tiles on screen waiting for a second request:
	// one column counted on a wide screen, or rows that only just fit.
	test("fill the screen's columns, plus half again of its rows", () => {
		expect(tileBatch(1280, 720, false)).toBe(6);
		expect(tileBatch(1920, 1080, false)).toBe(10);
		expect(tileBatch(390, 844, true)).toBe(3);
	});
});
