import "@bencherdev/ui/styles.css";
import "../styles/console.css";
import { render } from "solid-js/web";
import { afterEach, describe, expect, test } from "vitest";
import { page } from "vitest/browser";
import type { Tab } from "../paths";
import Shell from "./Shell";
import { centerCurrentTab } from "./scroll";

let dispose: (() => void) | undefined;

const mount = (tab: Tab = "reports") => {
	const root = document.createElement("div");
	root.className = "console";
	document.body.append(root);
	dispose = render(
		() => (
			<Shell
				slug="hashbrown"
				tab={tab}
				organization={{ name: "Pompeii LLC", uuid: "org-uuid" }}
				project="Hashbrown"
				alerts={1}
				account="/console/users/muriel-bagge/settings"
			/>
		),
		root,
	);
	return root;
};

const shown = (root: HTMLElement, selector: string) =>
	root.querySelector(selector)?.checkVisibility() ?? false;

const size = (root: HTMLElement, selector: string) => {
	const box = root.querySelector(selector)?.getBoundingClientRect();
	return { width: box?.width ?? 0, height: box?.height ?? 0 };
};

afterEach(() => {
	dispose?.();
	document.body.replaceChildren();
});

describe("narrow", () => {
	// Kills a narrow bar that keeps what it should fold away, or drops the bell.
	test("the bar keeps the mark, the project, the bell, and the avatar", async () => {
		await page.viewport(390, 844);
		const root = mount();

		for (const kept of [".brand", ".projlink", ".bell", ".avatar"]) {
			expect(shown(root, kept), kept).toBe(true);
		}
		for (const folded of [
			".brand-name",
			"a.crumb-org",
			".docs",
			".theme-toggle",
		]) {
			expect(shown(root, folded), folded).toBe(false);
		}
		await expect
			.element(page.getByRole("link", { name: "Alerts, 1 active" }))
			.toBeVisible();
	});

	// Kills touch targets under 44 px.
	test("every control in the bar and the tab row is a 44 px target", async () => {
		await page.viewport(390, 844);
		const root = mount();

		for (const target of [".brand", ".projlink", ".bell", ".avatar"]) {
			expect(size(root, target).height, target).toBeGreaterThanOrEqual(44);
			expect(size(root, target).width, target).toBeGreaterThanOrEqual(44);
		}
		for (const tab of root.querySelectorAll(".tab")) {
			const box = tab.getBoundingClientRect();
			expect(box.height).toBeGreaterThanOrEqual(44);
			expect(box.width).toBeGreaterThanOrEqual(44);
		}
	});

	// Kills a hit area that grows the mark, and a tab badge that crowds its tab.
	test("the mark and the tab badge keep their narrow sizes", async () => {
		await page.viewport(390, 844);
		const root = mount();

		expect(size(root, ".brand svg")).toEqual({ width: 28, height: 28 });
		expect(size(root, ".tab .badge").height).toBe(16);
	});

	// Kills a tab brought into view flush against the row's edge.
	test("a tab scrolled into view clears the row's edge", async () => {
		await page.viewport(390, 844);
		const root = mount();
		root.style.width = "200px";
		const tabs = root.querySelector<HTMLElement>(".tabs");
		const plots = page.getByRole("link", { name: "Plots" }).element();
		if (!tabs) {
			throw new Error("no tab row");
		}
		tabs.scrollLeft = tabs.scrollWidth;

		plots.scrollIntoView({ inline: "nearest", block: "nearest" });

		const gap =
			plots.getBoundingClientRect().left - tabs.getBoundingClientRect().left;
		expect(gap).toBeCloseTo(16, 0);
	});

	// Kills a scrolled row that leaves the current tab out of view, and one
	// centered against the page instead of the row.
	test("centering brings the current tab to the middle of the row", async () => {
		await page.viewport(390, 844);
		const root = mount("alerts");
		root.style.paddingLeft = "40px";
		const tabs = root.querySelector<HTMLElement>(".tabs");
		expect(tabs && tabs.scrollWidth > tabs.clientWidth).toBe(true);

		centerCurrentTab(tabs ?? undefined);

		const row = tabs?.getBoundingClientRect();
		const current = tabs
			?.querySelector('[aria-current="page"]')
			?.getBoundingClientRect();
		const middle = (box?: DOMRect) => (box ? box.left + box.width / 2 : 0);
		expect(Math.abs(middle(current) - middle(row))).toBeLessThan(1);
	});
});

describe("wide", () => {
	// Kills a wide bar that loses the organization, Docs, or the theme, or shows the bell.
	test("the bar names the organization and keeps Docs and the theme toggle", async () => {
		await page.viewport(1280, 800);
		const root = mount();

		for (const kept of [
			".brand-name",
			"a.crumb-org",
			".docs",
			".theme-toggle",
		]) {
			expect(shown(root, kept), kept).toBe(true);
		}
		expect(shown(root, ".bell")).toBe(false);
		await expect
			.element(page.getByRole("link", { name: "Alerts 1 active" }))
			.toBeVisible();
	});
});
