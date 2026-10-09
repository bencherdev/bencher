import { expect, test } from "vitest";
import { shortDate, shortDay } from "./time";

const NOW = new Date(2026, 8, 14).getTime();

// Kills a year shown on every date, or dropped from an older one.
test("a date names its year only outside this one", () => {
	expect(shortDay(new Date(2026, 2, 2).getTime(), NOW)).toBe("Mar 2");
	expect(shortDay(new Date(2025, 2, 2).getTime(), NOW)).toBe("Mar 2, 2025");
	expect(shortDate(new Date(2026, 8, 13, 21, 5).getTime(), NOW)).toBe(
		"Sep 13, 21:05",
	);
});
