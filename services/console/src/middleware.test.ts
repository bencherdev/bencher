import { readFileSync, readdirSync } from "node:fs";
import { expect, test } from "vitest";

// Kills a page under /auth/ or /console/ served as a file, which neither the
// Worker nor Node sends through the middleware, so it would carry none of the
// headers the middleware sets there.
test("every auth and console page renders per request", () => {
	for (const tree of ["auth", "console"]) {
		const dir = new URL(`./pages/${tree}/`, import.meta.url);
		const pages = readdirSync(dir, {
			recursive: true,
			encoding: "utf8",
		}).filter((file) => /\.(astro|ts)$/.test(file));
		expect(pages.length, tree).toBeGreaterThan(0);
		for (const file of pages) {
			expect(readFileSync(new URL(file, dir), "utf8"), file).toMatch(
				/^export const prerender = false;$/m,
			);
		}
	}
});
