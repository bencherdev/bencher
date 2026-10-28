// The console page's own styles, so a scan reads the contrast a reader sees.
import "@bencherdev/ui/styles.css";
import "../styles/console.css";
import "../explore/explore.css";
import "../reports/reports.css";
import "../plot/plot.css";
import "../report/report.css";
import "../thresholds/thresholds.css";
import "../dimensions/dimensions.css";
import "../alerts/alerts.css";
import "../plots/plots.css";
import "../public/public.css";
import "../feedback/feedback.css";
import "../switcher/switcher.css";
import { THEME_ATTRIBUTE } from "@bencherdev/ui/ThemeToggle";
import axe from "axe-core";
import { expect, vi } from "vitest";
import { page } from "vitest/browser";
import { AXE_OPTIONS, misnamed } from "./checks";

export const DESKTOP = 1280;
export const PHONE = 390;

/**
 * Scan what `open` mounts at each width with axe, once no animation runs:
 * on a desktop in the dark and the light theme, on a phone in the theme a
 * first visit gets. Each scan mounts afresh in its theme, so no transition
 * between the two is caught; `open` returns how to take it down.
 */
export const axeInBothThemes = async (
	open: (width: number) => Promise<() => void>,
	widths: number[] = [DESKTOP, PHONE],
) => {
	const root = document.documentElement;
	const [ownWidth, ownHeight] = [innerWidth, innerHeight];
	const found: string[] = [];
	try {
		for (const width of widths) {
			const themes =
				width > PHONE
					? ["dark", "light"]
					: [
							matchMedia("(prefers-color-scheme: light)").matches
								? "light"
								: "dark",
						];
			for (const theme of themes) {
				await page.viewport(width, 844);
				root.setAttribute(THEME_ATTRIBUTE, theme);
				const close = await open(width);
				await vi.waitFor(() =>
					expect(
						document
							.getAnimations()
							.every((animation) => animation.playState !== "running"),
					).toBe(true),
				);
				// The body, not the document: a page's own rules, such as its title and
				// its one main landmark, belong to the e2e scans of whole pages.
				const { violations } = await axe.run(document.body, AXE_OPTIONS);
				found.push(
					...[
						...violations.map(
							(violation) =>
								`${violation.id}: ${violation.nodes.map((node) => node.target.join(" ")).join(", ")}`,
						),
						...misnamed(),
					].map((each) => `${width} ${theme} ${each}`),
				);
				close();
			}
		}
	} finally {
		root.removeAttribute(THEME_ATTRIBUTE);
		await page.viewport(ownWidth, ownHeight);
	}
	expect(found).toEqual([]);
};
