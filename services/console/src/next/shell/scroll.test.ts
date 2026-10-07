import { describe, expect, test } from "vitest";
import { centeredScroll } from "./scroll";

describe("centeredScroll", () => {
	// Kills a narrow tab row that leaves the current tab out of view.
	test("centers the current tab when the row overflows", () => {
		expect(
			centeredScroll({
				scrollWidth: 600,
				clientWidth: 390,
				offsetLeft: 500,
				offsetWidth: 80,
			}),
		).toBe(345);
	});

	// Kills a row scrolled past its start for a tab near the start.
	test("never scrolls before the start", () => {
		expect(
			centeredScroll({
				scrollWidth: 600,
				clientWidth: 390,
				offsetLeft: 10,
				offsetWidth: 80,
			}),
		).toBe(0);
	});

	// Kills a wide row that scrolls at all.
	test("leaves a row that fits alone", () => {
		expect(
			centeredScroll({
				scrollWidth: 390,
				clientWidth: 390,
				offsetLeft: 300,
				offsetWidth: 80,
			}),
		).toBe(0);
	});
});
