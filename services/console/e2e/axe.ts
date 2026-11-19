import AxeBuilder from "@axe-core/playwright";
import type { Page } from "@playwright/test";
import { AXE_OPTIONS, misnamed } from "../src/next/axe/checks";
import { THEME_KEY, expect } from "./fixtures";

type AxePage = ConstructorParameters<typeof AxeBuilder>[0]["page"];

/** Whether going from `current` to `path` changes nothing but the fragment. */
const onlyTheFragmentDiffers = (current: string, path: string) => {
	const from = new URL(current);
	const to = new URL(path, current);
	return (
		to.hash !== "" &&
		from.origin === to.origin &&
		from.pathname === to.pathname &&
		from.search === to.search
	);
};

/** A phone, where a page folds its controls and may name them differently. */
const PHONE = { width: 390, height: 844 };

/**
 * Scan the page at `path` with axe once `ready` resolves and no animation
 * runs: a control easing between two states shows a contrast it never has at
 * rest. At the test's own width it scans in the dark and the light theme; on
 * a phone, in the theme a first visit gets, with none stored. With `arrive`,
 * each scan opens `path` and goes on from there, for a state no link holds.
 */
export const axeInBothThemes = async (
	page: Page,
	path: string,
	ready: () => Promise<void>,
	arrive?: () => Promise<void>,
) => {
	const own = page.viewportSize();
	const passes: [{ width: number; height: number }, string | null][] =
		own && own.width > PHONE.width
			? [
					[own, "dark"],
					[own, "light"],
					[PHONE, null],
				]
			: [[own ?? PHONE, null]];
	await page.goto(path);
	for (const [size, theme] of passes) {
		await page.setViewportSize(size);
		await page.evaluate(
			([key, value]) =>
				value === null
					? localStorage.removeItem(key)
					: localStorage.setItem(key, value),
			[THEME_KEY, theme] as const,
		);
		if (arrive) {
			// Going from a page to its own path with only another fragment loads no page, and a
			// page that reloads on that change would navigate under this one.
			if (onlyTheFragmentDiffers(page.url(), path)) {
				await page.goto("about:blank");
			}
			await page.goto(path);
			await arrive();
		} else {
			await page.reload();
		}
		await ready();
		const html = page.locator("html");
		await expect(html).toHaveAttribute("data-theme", theme ?? /^(dark|light)$/);
		await page.waitForFunction(() =>
			document
				.getAnimations()
				.every(
					(animation) =>
						animation.playState !== "running" ||
						!(animation.timeline instanceof DocumentTimeline),
				),
		);
		// The axe integration types `page` against its own copy of Playwright.
		const { violations } = await new AxeBuilder({
			page: page as unknown as AxePage,
		})
			.options(AXE_OPTIONS)
			.analyze();
		const shown = await html.getAttribute("data-theme");
		expect(
			[
				...violations.map(
					(violation) =>
						`${violation.id}: ${violation.nodes.map((node) => node.target.join(" ")).join(", ")}`,
				),
				...(await page.evaluate(misnamed)),
			].map((found) => `${size.width} ${shown} ${found}`),
		).toEqual([]);
	}
	if (own) {
		await page.setViewportSize(own);
	}
};
