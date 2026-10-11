import { readFileSync } from "node:fs";
import { experimental_AstroContainer as AstroContainer } from "astro/container";
import { Window } from "happy-dom";
import { describe, expect, test } from "vitest";
import {
	BENCHER_THEME_KEY,
	DATA_THEME,
} from "../components/navbar/theme/theme";
import Head from "./Head.astro";

const container = await AstroContainer.create();
const head = new new Window().DOMParser().parseFromString(
	await container.renderToString(Head),
	"text/html",
);

describe("the theme script", () => {
	const script = head.querySelector("script");

	// Runs the script on a fresh page whose storage holds `stored` (or throws it)
	// and whose system prefers `system`, and returns the theme it set.
	const themeFor = (
		stored: string | null | Error,
		system: "light" | "dark",
	): string | null => {
		const page = new Window();
		const localStorage = {
			getItem: (key: string) => {
				if (stored instanceof Error) {
					throw stored;
				}
				return key === BENCHER_THEME_KEY ? stored : null;
			},
		};
		const matchMedia = (query: string) => ({
			matches: query === `(prefers-color-scheme: ${system})`,
		});
		new Function(
			"localStorage",
			"matchMedia",
			"document",
			script?.textContent ?? "",
		)(localStorage, matchMedia, page.document);
		return page.document.documentElement.getAttribute(DATA_THEME);
	};

	test("runs inline, before the page can paint", () => {
		expect(script?.hasAttribute("src")).toBe(false);
		expect(script?.getAttribute("type")).toBeNull();
	});

	test.each([
		["light", "dark"],
		["dark", "light"],
	] as const)("a stored %s choice wins over a %s system", (stored, system) => {
		expect(themeFor(stored, system)).toBe(stored);
	});

	test.each(["light", "dark"] as const)(
		"with no stored choice, a %s system decides",
		(system) => {
			expect(themeFor(null, system)).toBe(system);
		},
	);

	test("a stored value that is not a theme falls back to the system", () => {
		expect(themeFor("sepia", "light")).toBe("light");
	});

	test("blocked storage falls back to the system", () => {
		expect(themeFor(new Error("storage is blocked"), "light")).toBe("light");
	});
});

test("preloads exactly the font file the stylesheet loads", () => {
	const fonts = readFileSync(
		new URL("../../../../packages/ui/src/styles/fonts.css", import.meta.url),
		"utf8",
	);
	const files = [...fonts.matchAll(/url\("\.\.\/fonts\/([^"]+)"\)/g)].map(
		([, file]) => file,
	);
	const preloads = [
		...head.querySelectorAll('link[rel="preload"][as="font"]'),
	].map((link) => ({
		file: link.getAttribute("href")?.split("/").pop(),
		type: link.getAttribute("type"),
		crossorigin: link.hasAttribute("crossorigin"),
	}));
	expect(preloads).toEqual(
		files.map((file) => ({ file, type: "font/woff2", crossorigin: true })),
	);
});
