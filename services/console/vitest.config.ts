/// <reference types="vitest/config" />
import { playwright } from "@vitest/browser-playwright";
import { getViteConfig } from "astro/config";
import solid from "vite-plugin-solid";
import { configDefaults } from "vitest/config";
import type { BrowserCommand } from "vitest/node";

const CHROMIUM_TESTS = "src/**/*.chromium.test.{ts,tsx}";

/** Rest the mouse at the page's corner, where a fresh page has it. */
const restPointer: BrowserCommand<[]> = async ({ page }) => {
	await page.mouse.move(0, 0);
};

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
						allow: [".", "../../packages/ui"],
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
						commands: { restPointer },
					},
				},
			},
		],
	},
});
