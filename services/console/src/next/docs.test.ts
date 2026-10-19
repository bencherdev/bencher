import { readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { expect, test } from "vitest";

const CONSOLE = fileURLToPath(new URL("../../", import.meta.url));
const ROOTS = ["src/next", "src/pages/next"];
const SOURCE = /\.(?:astro|mjs|tsx?)$/;
const TEST = /\.test\.[jt]sx?$/;
// A self-hosted install without internet serves its own docs, so no link is
// built from bencher.dev's address.
const ABSOLUTE_SITE = /(?:https?:)?\/\/(?:www\.)?bencher\.dev\b/;

const walk = (dir: string): string[] =>
	readdirSync(join(CONSOLE, dir), { withFileTypes: true }).flatMap((entry) => {
		const path = `${dir}/${entry.name}`;
		if (entry.isDirectory()) {
			return walk(path);
		}
		return SOURCE.test(path) && !TEST.test(path) ? [path] : [];
	});

// Kills a docs link, written whole or built from the site's address, that
// leaves a self-hosted install for bencher.dev.
test("every docs link in the new console is relative", () => {
	expect(
		ROOTS.flatMap(walk).flatMap((path) =>
			readFileSync(join(CONSOLE, path), "utf8")
				.split("\n")
				.flatMap((line, index) =>
					ABSOLUTE_SITE.test(line) ? [`${path}:${index + 1}`] : [],
				),
		),
	).toEqual([]);
});
