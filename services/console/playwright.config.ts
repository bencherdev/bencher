import { readFileSync } from "node:fs";
import { defineConfig, devices } from "@playwright/test";

// `cargo test-console e2e` starts the API and the console, seeds the API, and
// points this at the seed it wrote.
const seed = process.env.BENCHER_E2E_SEED;
const baseURL = seed
	? JSON.parse(readFileSync(seed, "utf8")).console_url
	: undefined;

export default defineConfig({
	testDir: "./e2e",
	testMatch: "**/*.e2e.ts",
	forbidOnly: !!process.env.CI,
	retries: 0,
	reporter: process.env.CI ? [["list"], ["github"]] : "list",
	use: {
		baseURL,
		trace: "retain-on-failure",
	},
	projects: [{ name: "chromium", use: { ...devices["Desktop Chrome"] } }],
});
