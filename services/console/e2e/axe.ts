import AxeBuilder from "@axe-core/playwright";
import type { Page } from "@playwright/test";
import { THEME_KEY, expect } from "./fixtures";

type AxePage = ConstructorParameters<typeof AxeBuilder>[0]["page"];

/** A phone, where a page folds its controls and may name them differently. */
const PHONE = { width: 390, height: 844 };

/**
 * Scan the page at `path` with axe in the dark and the light theme, at the
 * test's own width and on a phone, once `ready` resolves in each and no
 * animation runs: a control easing between two states shows a contrast it
 * never has at rest. With `arrive`, each scan opens `path` and goes on from
 * there, for a state no link holds.
 */
export const axeInBothThemes = async (
	page: Page,
	path: string,
	ready: () => Promise<void>,
	arrive?: () => Promise<void>,
) => {
	const own = page.viewportSize();
	const sizes = own && own.width > PHONE.width ? [own, PHONE] : [own ?? PHONE];
	await page.goto(path);
	for (const size of sizes) {
		await page.setViewportSize(size);
		for (const theme of ["dark", "light"]) {
			await page.evaluate(([key, value]) => localStorage.setItem(key, value), [
				THEME_KEY,
				theme,
			] as const);
			if (arrive) {
				await page.goto(path);
				await arrive();
			} else {
				await page.reload();
			}
			await ready();
			await expect(page.locator("html")).toHaveAttribute("data-theme", theme);
			await page.waitForFunction(() =>
				document
					.getAnimations()
					.every((animation) => animation.playState !== "running"),
			);
			// The axe integration types `page` against its own copy of Playwright.
			const { violations } = await new AxeBuilder({
				page: page as unknown as AxePage,
			})
				// Off by default in axe; a control's name starts with its visible label.
				.options({
					rules: { "label-content-name-mismatch": { enabled: true } },
				})
				.analyze();
			expect(
				[
					...violations.map(
						(violation) =>
							`${violation.id}: ${violation.nodes.map((node) => node.target.join(" ")).join(", ")}`,
					),
					...(await misnamed(page)),
				].map((found) => `${size.width} ${theme} ${found}`),
			).toEqual([]);
		}
	}
	if (own) {
		await page.setViewportSize(own);
	}
};

/**
 * Each control whose accessible name does not start with the text it shows,
 * both read as words as axe reads them: its label-in-name rule only asks that
 * the name contain the text.
 */
const misnamed = (page: Page) =>
	page.evaluate(() => {
		const hidden = (element: Element) =>
			!element.checkVisibility({ visibilityProperty: true }) ||
			element.closest(".sr-only") !== null;
		const words = (text: string) =>
			text
				.replace(/[^\p{L}\p{N}]+/gu, " ")
				.trim()
				.toLowerCase();
		const shown = (node: Node): string =>
			node.nodeType === Node.TEXT_NODE
				? (node.nodeValue ?? "")
				: !(node instanceof Element) || hidden(node)
					? ""
					: node.nodeName === "BR"
						? " "
						: [...node.childNodes]
								.map(shown)
								.join("")
								.replace(/\s+/g, " ")
								.trim();
		const controls = document.querySelectorAll(
			'a[href], button, summary, [role="button"], [role="link"], [role="tab"], [role^="menuitem"], [role="checkbox"], [role="radio"], [role="switch"], [role="option"]',
		);
		return [...controls].flatMap((control) => {
			const labelledBy = control.getAttribute("aria-labelledby");
			const name = labelledBy
				? labelledBy
						.split(/\s+/)
						.map((id) => document.getElementById(id)?.textContent ?? "")
						.join(" ")
				: control.getAttribute("aria-label");
			const text = shown(control);
			if (name === null || hidden(control) || words(text) === "") {
				return [];
			}
			return `${words(name)} `.startsWith(`${words(text)} `)
				? []
				: [`name-starts-with-text: "${text}" is named "${name}"`];
		});
	});
