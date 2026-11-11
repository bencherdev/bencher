/// <reference types="vitest/config" />
import { playwright } from "@vitest/browser-playwright";
import { getViteConfig } from "astro/config";
import solid from "vite-plugin-solid";
import { configDefaults } from "vitest/config";
import type { CDPSession } from "playwright";
import type { BrowserCommand } from "vitest/node";

const CHROMIUM_TESTS = "src/**/*.chromium.test.{ts,tsx}";

/** Rest the mouse at the page's corner, where a fresh page has it, its button up. */
const restPointer: BrowserCommand<[]> = async ({ page }) => {
	await page.mouse.up();
	await page.mouse.move(0, 0);
};

/** Press the mouse's main button where the mouse is. */
const mouseDown: BrowserCommand<[]> = async ({ page }) => {
	await page.mouse.down();
};

/** Release the mouse's main button where the mouse is. */
const mouseUp: BrowserCommand<[]> = async ({ page }) => {
	await page.mouse.up();
};

/**
 * A touch event the browser takes as real fingers, each point in the test
 * page's CSS pixels. The browser works by difference from the fingers already
 * down: `touchStart` presses only the points not yet down, `touchMove` moves
 * the points given and presses any not yet down as new fingers, `touchEnd`
 * lifts the points given, or every finger when given none, and
 * `touchCancel` cancels every finger.
 */
const touch: BrowserCommand<
	[
		type: "touchStart" | "touchMove" | "touchEnd" | "touchCancel",
		points: { id: number; x: number; y: number }[],
	]
> = async ({ page, frame }, type, points) => {
	const inner = await frame();
	const box = await (await inner.frameElement()).boundingBox();
	const width = await inner.evaluate(() => window.innerWidth);
	const scale = box ? box.width / width : 1;
	let session = sessions.get(page);
	if (!session) {
		session = await page.context().newCDPSession(page);
		sessions.set(page, session);
	}
	await session.send("Input.dispatchTouchEvent", {
		type,
		touchPoints: points.map(({ id, x, y }) => ({
			id,
			x: (box?.x ?? 0) + x * scale,
			y: (box?.y ?? 0) + y * scale,
		})),
	});
};
// The browser holds the fingers down per session, so every touch of a page goes through one.
const sessions = new WeakMap<object, CDPSession>();

/** The accessibility tree under `selector` as Playwright reads it, names and all. */
const ariaSnapshot: BrowserCommand<[selector: string]> = async (
	{ frame },
	selector,
) => (await frame()).locator(selector).ariaSnapshot();

export default getViteConfig({
	test: {
		/* for example, use global to avoid globals imports (describe, test, expect): */
		// globals: true,
		includeSource: ["../../lib/bencher_valid"],
		projects: [
			{
				extends: true,
				test: {
					name: "unit",
					exclude: [...configDefaults.exclude, CHROMIUM_TESTS],
				},
			},
			{
				// Solid alone, since the site's WASM plugin fails when two projects start it at once.
				plugins: [solid()],
				resolve: {
					dedupe: ["solid-js", "solid-js/web", "solid-js/store"],
				},
				server: {
					fs: {
						// The MCP App's test reads the page the MCP server serves.
						allow: [".", "../../packages/ui", "../../plus/api_mcp/ui"],
					},
				},
				test: {
					name: "browser",
					include: [CHROMIUM_TESTS],
					provide: {
						speedFactor: Number(process.env.BENCHER_SPEED_FACTOR ?? 1),
					},
					setupFiles: ["src/next/pointer.setup.ts"],
					browser: {
						enabled: true,
						provider: playwright(),
						headless: true,
						instances: [{ browser: "chromium" }],
						commands: { restPointer, mouseDown, mouseUp, touch, ariaSnapshot },
					},
				},
			},
		],
	},
});
