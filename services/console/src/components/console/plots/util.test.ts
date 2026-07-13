import { describe, expect, test } from "vitest";
import { PLOT_PRELOAD_MARGIN, isNearViewport } from "./util";

const VIEWPORT_HEIGHT = 800;

describe("isNearViewport", () => {
	test("fully within the viewport is near", () => {
		expect(isNearViewport({ top: 100, bottom: 500 }, VIEWPORT_HEIGHT)).toBe(
			true,
		);
	});

	test("flush with the top of the viewport is near", () => {
		expect(isNearViewport({ top: 0, bottom: 584 }, VIEWPORT_HEIGHT)).toBe(true);
	});

	test("straddling the bottom fold is near", () => {
		expect(isNearViewport({ top: 700, bottom: 1200 }, VIEWPORT_HEIGHT)).toBe(
			true,
		);
	});

	test("just below the fold but within the preload margin is near", () => {
		// top is below the viewport but within PLOT_PRELOAD_MARGIN (200)
		expect(
			isNearViewport(
				{ top: VIEWPORT_HEIGHT + 100, bottom: VIEWPORT_HEIGHT + 600 },
				VIEWPORT_HEIGHT,
			),
		).toBe(true);
	});

	test("far below the fold beyond the margin is not near", () => {
		expect(isNearViewport({ top: 2000, bottom: 2500 }, VIEWPORT_HEIGHT)).toBe(
			false,
		);
	});

	test("just above the viewport but within the preload margin is near", () => {
		expect(isNearViewport({ top: -150, bottom: -100 }, VIEWPORT_HEIGHT)).toBe(
			true,
		);
	});

	test("far above the viewport beyond the margin is not near", () => {
		expect(isNearViewport({ top: -800, bottom: -300 }, VIEWPORT_HEIGHT)).toBe(
			false,
		);
	});

	test("a not-yet-laid-out zero rect is treated as near (safe default: load)", () => {
		// getBoundingClientRect can return zeros before layout; prefer loading
		// over leaving the plot stuck on the skeleton.
		expect(isNearViewport({ top: 0, bottom: 0 }, VIEWPORT_HEIGHT)).toBe(true);
	});

	test("respects a custom zero margin at the exact fold", () => {
		// With no margin, an element whose top is exactly at the fold is below it.
		expect(
			isNearViewport(
				{ top: VIEWPORT_HEIGHT, bottom: VIEWPORT_HEIGHT + 50 },
				VIEWPORT_HEIGHT,
				0,
			),
		).toBe(false);
		expect(
			isNearViewport(
				{ top: VIEWPORT_HEIGHT - 1, bottom: VIEWPORT_HEIGHT + 50 },
				VIEWPORT_HEIGHT,
				0,
			),
		).toBe(true);
	});

	test("preload margin constant is a positive number of pixels", () => {
		expect(PLOT_PRELOAD_MARGIN).toBeGreaterThan(0);
	});
});
