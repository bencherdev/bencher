import AxeBuilder from "@axe-core/playwright";
import type { Page } from "@playwright/test";
import { THEME_KEY, expect } from "./fixtures";

type AxePage = ConstructorParameters<typeof AxeBuilder>[0]["page"];

/**
 * Scan the page at `path` with axe in the dark and the light theme, after
 * `ready` resolves in each.
 */
export const axeInBothThemes = async (
	page: Page,
	path: string,
	ready: () => Promise<void>,
) => {
	await page.goto(path);
	for (const theme of ["dark", "light"]) {
		await page.evaluate(([key, value]) => localStorage.setItem(key, value), [
			THEME_KEY,
			theme,
		] as const);
		await page.reload();
		await ready();
		await expect(page.locator("html")).toHaveAttribute("data-theme", theme);
		// The axe integration types `page` against its own copy of Playwright.
		const { violations } = await new AxeBuilder({
			page: page as unknown as AxePage,
		}).analyze();
		expect(
			violations.map(
				(violation) =>
					`${theme} ${violation.id}: ${violation.nodes.map((node) => node.target.join(" ")).join(", ")}`,
			),
		).toEqual([]);
	}
};
