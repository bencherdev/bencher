import { describe, expect, test } from "vitest";
import { batchSize, hasMore, loadMore, visibleRange } from "./rows";

describe("batchSize", () => {
	// Kills a batch that is not sized from the screen, that leaves no margin
	// below it, or that asks for more than the API pages.
	test.each([
		[720, 40, 27],
		[600, 40, 23],
		[844, 60, 23],
		[1440, 40, 54],
		// The API pages at most 255.
		[100_000, 40, 255],
	])("a %i px screen of %i px rows asks for %i", (screen, row, batch) => {
		expect(batchSize(screen, row)).toBe(batch);
	});

	// Kills a batch of none before the window has a height.
	test("a screen with no height still asks for a screen's worth", () => {
		expect(batchSize(0, 40)).toBe(batchSize(720, 40));
	});
});

describe("visibleRange", () => {
	const range = (top: number, count = 100) =>
		visibleRange({ top, viewport: 400, rowHeight: 40, count, overscan: 2 });

	// Kills rows drawn from the top of the list once it scrolls, a range that
	// ignores the overscan, and one that runs past the rows loaded.
	test.each([
		// The rows start 200 px down the screen: five fit, plus the overscan.
		[200, { start: 0, end: 7 }],
		// The rows start at the top of the screen.
		[0, { start: 0, end: 12 }],
		// Scrolled 1,000 px into the rows: rows 25 to 34 are on screen.
		[-1_000, { start: 23, end: 37 }],
		// Scrolled 1,020 px: half of row 25 is still on screen.
		[-1_020, { start: 23, end: 38 }],
		// Scrolled past the end.
		[-5_000, { start: 100, end: 100 }],
		// The rows are below the screen.
		[900, { start: 0, end: 0 }],
	])("rows starting at %i px draw %o", (top, expected) => {
		expect(range(top)).toEqual(expected);
	});

	test("no rows draw nothing", () => {
		expect(range(0, 0)).toEqual({ start: 0, end: 0 });
	});
});

// Kills a next batch asked for too late to arrive before the reader reaches the
// end, or asked for while a screen and more of loaded rows still lie below.
test("loadMore once the last row on screen comes within a quarter batch of the end", () => {
	expect(loadMore({ end: 10, loaded: 27, batch: 27 })).toBe(false);
	expect(loadMore({ end: 20, loaded: 27, batch: 27 })).toBe(false);
	expect(loadMore({ end: 21, loaded: 27, batch: 27 })).toBe(true);
	expect(loadMore({ end: 27, loaded: 27, batch: 27 })).toBe(true);
});

// Kills paging past the total, and past a batch shorter than asked for (the
// API drops a report it cannot read, so the total can overstate the rows).
test("hasMore only after a full batch that leaves rows to load", () => {
	const batch = (length: number, total: number) => ({
		reports: Array.from({ length }),
		total,
		batch: { perPage: 27 },
	});
	expect(hasMore(batch(27, 54), 27)).toBe(true);
	expect(hasMore(batch(27, 54), 54)).toBe(false);
	expect(hasMore(batch(27, 27), 27)).toBe(false);
	expect(hasMore(batch(26, 54), 26)).toBe(false);
});
