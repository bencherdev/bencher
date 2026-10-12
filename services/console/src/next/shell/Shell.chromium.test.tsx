import "@bencherdev/ui/styles.css";
import "../styles/console.css";
import { createSignal } from "solid-js";
import { render } from "solid-js/web";
import { afterEach, describe, expect, test } from "vitest";
import { page, userEvent } from "vitest/browser";
import type { Tab } from "../paths";
import { expectNameStartsWithText, shownText } from "../ui/testing";
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

const size = (root: HTMLElement, selector: string) => {
	const box = root.querySelector(selector)?.getBoundingClientRect();
	return { width: box?.width ?? 0, height: box?.height ?? 0 };
};

/** What the bar shows a reader, each control by its name, in order. */
const held = (root: HTMLElement) =>
	[...root.querySelectorAll<HTMLElement>(".bar a, .bar button")]
		.filter((control) => control.checkVisibility())
		.map((control) => control.getAttribute("aria-label") ?? shownText(control));

/** The bar's controls a reader can see that are drawn outside it. */
const strays = (root: HTMLElement) => {
	const bar = root.querySelector(".bar")?.getBoundingClientRect();
	return [...root.querySelectorAll<HTMLElement>(".bar a, .bar button")]
		.filter((control) => {
			const { top, bottom } = control.getBoundingClientRect();
			return (
				control.checkVisibility() &&
				(top < (bar?.top ?? 0) || bottom > (bar?.bottom ?? 0))
			);
		})
		.map((control) => control.getAttribute("aria-label") ?? shownText(control));
};

/** The wordmark images drawn, by path. */
const wordmark = (root: HTMLElement) =>
	[...root.querySelectorAll<HTMLImageElement>(".brand img")]
		.filter((image) => image.checkVisibility())
		.map((image) => new URL(image.src).pathname);

/** The drawn wordmark's box. */
const box = (root: HTMLElement) => {
	const shown = [...root.querySelectorAll(".brand img")].find((image) =>
		image.checkVisibility(),
	);
	const { width, height } = shown?.getBoundingClientRect() ?? {};
	return { width, height };
};

const html = document.documentElement;

afterEach(() => {
	dispose?.();
	document.body.replaceChildren();
	html.removeAttribute("data-theme");
	localStorage.clear();
});

describe("narrow", () => {
	// Kills a phone bar that shows the project or the organization in place of
	// Bencher, keeps Docs or the theme toggle out of the menu, or drops the bell.
	test("the bar holds the wordmark, the bell, the menu, and the avatar", async () => {
		await page.viewport(390, 844);
		const root = mount();

		expect(held(root)).toEqual([
			"Bencher, home",
			"1 active alert",
			"Menu",
			"Account",
		]);
		expect(strays(root)).toEqual([]);
		const bell = page.getByRole("link", { name: "1 active alert" });
		await expect.element(bell).toBeVisible();
		expectNameStartsWithText(bell.element());
	});

	// Kills touch targets under 44 px, in the bar, the open menu, and the tab
	// row, and a menu that opens off the screen, over the bar, or away from its
	// button, such as without its insets (it falls below the screen), at the
	// top (over the bar), or past the left edge.
	test("every control in the bar, the menu, and the tab row is a 44 px target, and the menu opens on screen under its button", async () => {
		await page.viewport(390, 844);
		const root = mount();
		await page.getByRole("button", { name: "Menu" }).click();
		await expect
			.element(page.getByRole("link", { name: "Docs" }))
			.toBeVisible();

		const bar = root.querySelector(".bar")?.getBoundingClientRect();
		const button = root.querySelector(".tools-menu")?.getBoundingClientRect();
		const menu = root.querySelector("#tools")?.getBoundingClientRect();
		if (!(bar && button && menu)) {
			throw new Error("no bar, menu button, or menu");
		}
		expect(menu.top).toBeGreaterThanOrEqual(bar.bottom);
		expect(menu.top).toBeLessThanOrEqual(bar.bottom + 8);
		expect(menu.left).toBeLessThanOrEqual(button.left);
		expect(menu.right).toBeGreaterThanOrEqual(button.right);
		expect(menu.left).toBeGreaterThanOrEqual(0);
		expect(menu.right).toBeLessThanOrEqual(window.innerWidth);
		expect(menu.bottom).toBeLessThanOrEqual(window.innerHeight);

		const targets = [
			...root.querySelectorAll(".bar a, .bar button, .tab"),
		].filter((control) => control.checkVisibility());
		expect(targets).toHaveLength(4 + 2 + 6);
		for (const control of targets) {
			const target = control.getBoundingClientRect();
			const name = control.getAttribute("aria-label") ?? shownText(control);
			expect(target.height, name).toBeGreaterThanOrEqual(44);
			expect(target.width, name).toBeGreaterThanOrEqual(44);
		}
	});

	// Kills a hit area that grows the wordmark, a narrow size given to one
	// theme's wordmark only, and a tab badge that crowds its tab.
	test("the wordmark and the tab badge keep their narrow sizes", async () => {
		await page.viewport(390, 844);
		const root = mount();

		expect(box(root)).toEqual({ width: 106, height: 20 });
		html.setAttribute("data-theme", "light");
		expect(box(root)).toEqual({ width: 106, height: 20 });
		expect(size(root, ".tab .badge").height).toBe(16);
	});

	// Kills a menu the keyboard cannot open or enter, an Escape that does not
	// close it or hand the focus back, which strands a reader in a hidden menu,
	// and a menu opened by script, whose button no longer reports it expanded.
	test("the menu opens on Enter and Space, Tab enters it, and Escape gives the focus back", async () => {
		await page.viewport(390, 844);
		mount();
		const menu = page.getByRole("button", { name: "Menu" });
		const docs = page.getByRole("link", { name: "Docs" });
		const toggle = page.getByRole("button", {
			name: "Switch between light and dark",
		});
		expect((menu.element() as HTMLButtonElement).popoverTargetElement).toBe(
			document.getElementById("tools"),
		);

		for (const key of ["{Enter}", " "]) {
			(menu.element() as HTMLElement).focus();
			await userEvent.keyboard(key);
			await expect.element(toggle).toBeVisible();
			await userEvent.keyboard("{Tab}");
			await expect.element(docs).toHaveFocus();
			await userEvent.keyboard("{Tab}");
			await expect.element(toggle).toHaveFocus();
			const shown = docs.element();
			await userEvent.keyboard("{Escape}");
			await expect.element(menu).toHaveFocus();
			expect(shown.checkVisibility()).toBe(false);
		}
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
	// Kills a wide bar that loses the crumbs, Docs, or the theme toggle, draws
	// Docs and the toggle anywhere but in the bar, shows the bell beside the
	// Alerts tab's count, or shows the phone's menu button.
	test("the bar holds the wordmark, the crumbs, Docs, the theme toggle, and the avatar", async () => {
		await page.viewport(1280, 800);
		const root = mount();

		expect(held(root)).toEqual([
			"Bencher, home",
			"Pompeii LLC",
			"Hashbrown",
			"Docs",
			"Switch between light and dark",
			"Account",
		]);
		expect(strays(root)).toEqual([]);
		await expect
			.element(page.getByRole("link", { name: "Alerts 1 active" }))
			.toBeVisible();
	});

	// Kills a wordmark that follows the reader's system rather than the page's
	// theme, both drawn at once, or a theme with none.
	test("the wordmark follows the page's theme, set or toggled", async () => {
		await page.viewport(1280, 800);
		const root = mount();

		expect(wordmark(root)).toEqual(["/wordmark-dark.svg"]);
		html.setAttribute("data-theme", "light");
		expect(wordmark(root)).toEqual(["/wordmark-light.svg"]);
		html.setAttribute("data-theme", "dark");
		expect(wordmark(root)).toEqual(["/wordmark-dark.svg"]);
		await page
			.getByRole("button", { name: "Switch between light and dark" })
			.click();
		expect(wordmark(root)).toEqual(["/wordmark-light.svg"]);
	});

	// Kills a wordmark sized by its image, which moves the bar when the image
	// arrives, one a long name squeezes, and a theme swap that draws the other
	// wordmark at another size.
	test("the wordmark holds its size with no image yet, beside long names, and across a theme swap", async () => {
		await page.viewport(768, 800);
		const root = document.createElement("div");
		root.className = "console";
		document.body.append(root);
		dispose = render(
			() => (
				<Shell
					slug="hashbrown"
					tab="reports"
					organization={{ name: "Pompeii LLC ".repeat(4), uuid: "org-uuid" }}
					project={"Hashbrown ".repeat(12)}
					alerts={1}
				/>
			),
			root,
		);
		const reserved = { width: 150, height: 28.25 };

		expect(box(root)).toEqual(reserved);
		const images = [...root.querySelectorAll<HTMLImageElement>(".brand img")];
		for (const image of images) {
			image.removeAttribute("src");
		}
		await expect
			.poll(() =>
				images.every((image) => image.complete && !image.naturalWidth),
			)
			.toBe(true);
		expect(box(root)).toEqual(reserved);
		html.setAttribute("data-theme", "light");
		expect(box(root)).toEqual(reserved);
	});

	// Kills a badge run into the tab's label ("Alerts1"): voice control and the
	// label-in-name check read the text as written, so the name must start with it.
	test("the Alerts tab's name starts with the text it shows", async () => {
		await page.viewport(1280, 800);
		const root = mount();

		const tab = root.querySelector('.tab[data-shell="alerts"]');
		expect(tab && shownText(tab)).toBe("Alerts 1");
		expectNameStartsWithText(tab);
	});
});

// Kills an Alerts tab that grows when its count arrives, or shrinks for a
// project with none, which moves every tab after it, whatever the count.
test("the Alerts tab holds its width before, with, and without a count", async () => {
	for (const width of [1280, 390]) {
		await page.viewport(width, 800);
		const [alerts, setAlerts] = createSignal<number | undefined>();
		const root = document.createElement("div");
		root.className = "console";
		document.body.append(root);
		dispose = render(
			() => <Shell slug="hashbrown" tab="reports" alerts={alerts()} />,
			root,
		);
		const tab = () =>
			root.querySelector('.tab[data-shell="alerts"]')?.getBoundingClientRect()
				.width;
		const before = tab();
		for (const count of [3, 12, 99, 100, 0]) {
			setAlerts(count);
			expect(tab(), `${count} at ${width}`).toBe(before);
		}
		dispose();
		document.body.replaceChildren();
	}
});

// Kills an empty badge drawn beside "Alerts" for a project with no alerts,
// and a count past 99 shown or named as anything but 99+, whose name then
// would not start with what the tab and the bell show.
test("the Alerts tab draws no badge without a count, and says 99+ past 99", async () => {
	await page.viewport(390, 800);
	const [alerts, setAlerts] = createSignal<number | undefined>();
	const root = document.createElement("div");
	root.className = "console";
	document.body.append(root);
	dispose = render(
		() => <Shell slug="hashbrown" tab="reports" alerts={alerts()} />,
		root,
	);
	const tab = () => root.querySelector('.tab[data-shell="alerts"]');
	const shownBadge = () =>
		tab()?.querySelector(".badge")?.checkVisibility({
			visibilityProperty: true,
		});
	expect(shownBadge()).toBe(false);
	setAlerts(0);
	expect(shownBadge()).toBe(false);
	setAlerts(120);
	expect(shownBadge()).toBe(true);
	expect(shownText(tab() as Element)).toBe("Alerts 99+");
	expectNameStartsWithText(tab());
	expectNameStartsWithText(root.querySelector(".bell"));
});
