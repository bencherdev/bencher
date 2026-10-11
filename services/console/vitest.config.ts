/// <reference types="vitest/config" />
import { playwright } from "@vitest/browser-playwright";
import { getViteConfig } from "astro/config";
import solid from "vite-plugin-solid";
import { configDefaults } from "vitest/config";

const CHROMIUM_TESTS = "src/**/*.chromium.test.{ts,tsx}";

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
					browser: {
						enabled: true,
						provider: playwright(),
						headless: true,
						instances: [{ browser: "chromium" }],
					},
				},
			},
		],
	},
});
