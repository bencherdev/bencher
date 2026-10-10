import "@bencherdev/ui/styles.css";
import "./plot.css";
import { THEME_ATTRIBUTE } from "@bencherdev/ui/ThemeToggle";
import { For } from "solid-js";
import { render } from "solid-js/web";
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { page } from "vitest/browser";
import InlineHistory from "./InlineHistory";
import { near, pixelAt, rowsOf, settle, tokenRgb } from "./pixels";
import type { InlineLine } from "./inline";

let dispose: (() => void) | undefined;

beforeEach(async () => {
	await page.viewport(1280, 900);
	document.documentElement.setAttribute(THEME_ATTRIBUTE, "dark");
});

afterEach(() => {
	dispose?.();
	dispose = undefined;
	document.body.replaceChildren();
	document.documentElement.removeAttribute(THEME_ATTRIBUTE);
});

const X = [0, 1, 2, 3, 4];

// A flat line under a flat upper limit, alerting at its last point.
const ALERTING: InlineLine = {
	y: [10, 10, 10, 10, 10],
	upper: [12, 12, 12, 12, 12],
	alerts: [4],
};

const mount = (line: InlineLine, muted = false) => {
	const root = document.createElement("div");
	document.body.append(root);
	dispose = render(
		() => (
			<InlineHistory
				x={X}
				line={line}
				muted={muted}
				label="History over 4 weeks"
			/>
		),
		root,
	);
};

const canvasNamed = (name: string) => {
	const canvas = page.getByRole("img", { name }).element();
	if (!(canvas instanceof HTMLCanvasElement)) {
		throw new Error("The history is not a canvas");
	}
	return { canvas, box: canvas.getBoundingClientRect() };
};

// Kills a history drawn in the wrong color, without its band, or without its marker.
test("draws the line, its band below the limit, and its alert marker", async () => {
	mount(ALERTING);
	await settle();
	const { canvas, box } = canvasNamed("History over 4 weeks");
	const series = tokenRgb("--color-data-categorical-1");
	const column = box.left + box.width / 3;
	const line = rowsOf(canvas, column, box.top, box.bottom, series);
	expect(line).not.toEqual([]);

	const below = pixelAt(canvas, column, box.bottom - 7);
	expect(near(below.rgb, series)).toBe(true);
	expect(Math.abs(below.alpha - 41)).toBeLessThan(14);
	expect(pixelAt(canvas, column, box.top + 2).alpha).toBe(0);

	const marker = tokenRgb("--color-marker");
	const last = box.right - 8;
	expect(
		rowsOf(canvas, last - 2, box.top, box.bottom, marker).length,
	).toBeGreaterThan(0);
});

test("draws a dismissed row's history muted", async () => {
	mount(ALERTING, true);
	await settle();
	const { canvas, box } = canvasNamed("History over 4 weeks");
	const column = box.left + box.width / 3;
	expect(
		rowsOf(canvas, column, box.top, box.bottom, tokenRgb("--color-text-muted")),
	).not.toEqual([]);
	expect(
		rowsOf(
			canvas,
			column,
			box.top,
			box.bottom,
			tokenRgb("--color-data-categorical-1"),
		),
	).toEqual([]);
});

test("redraws when the theme changes", async () => {
	mount(ALERTING);
	await settle();
	const { canvas, box } = canvasNamed("History over 4 weeks");
	const column = box.left + box.width / 3;
	const dark = tokenRgb("--color-data-categorical-1");

	document.documentElement.setAttribute(THEME_ATTRIBUTE, "light");
	await settle();

	const light = tokenRgb("--color-data-categorical-1");
	expect(near(dark, light)).toBe(false);
	expect(rowsOf(canvas, column, box.top, box.bottom, light)).not.toEqual([]);
});

// Kills a media query listener per row, which hundreds of rows would multiply.
test("shares one media query listener across rows", () => {
	const listen = vi.spyOn(MediaQueryList.prototype, "addEventListener");
	const root = document.createElement("div");
	document.body.append(root);
	try {
		dispose = render(
			() => (
				<For each={Array.from({ length: 50 })}>
					{() => <InlineHistory x={X} line={ALERTING} label="History" />}
				</For>
			),
			root,
		);
		expect(
			listen.mock.calls.filter(([type]) => type === "change").length,
		).toBeLessThanOrEqual(1);
	} finally {
		listen.mockRestore();
	}
});

// Kills a narrow signal that is stale at first use, which draws a phone's rows into a desktop-sized bitmap.
// It holds only because the viewport changes while nothing is mounted: beforeEach sets 1280 before this test sets 414.
test("draws a phone's history at the narrow size", async () => {
	await page.viewport(414, 896);
	mount(ALERTING);
	await settle();
	const { canvas } = canvasNamed("History over 4 weeks");
	expect(canvas.width).toBe(Math.round(112 * devicePixelRatio));
});
