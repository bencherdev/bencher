/// <reference types="vitest/config" />
import { playwright } from "@vitest/browser-playwright";
import { getViteConfig } from "astro/config";
import solid from "vite-plugin-solid";
import { configDefaults } from "vitest/config";
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
						commands: { restPointer, mouseDown, mouseUp, ariaSnapshot },
					},
				},
			},
		],
	},
});
