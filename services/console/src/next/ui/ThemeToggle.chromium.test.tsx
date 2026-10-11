import "@bencherdev/ui/styles.css";
import ThemeToggle from "@bencherdev/ui/ThemeToggle";
import { render } from "solid-js/web";
import { afterEach, beforeEach, expect, test } from "vitest";
import { page } from "vitest/browser";
import {
	BENCHER_THEME_KEY,
	DATA_THEME,
} from "../../components/navbar/theme/theme";
import { THEME_KEY } from "../theme";

let dispose: (() => void) | undefined;

const mountToggle = () => {
	const root = document.createElement("div");
	document.body.append(root);
	dispose = render(() => <ThemeToggle storageKey={THEME_KEY} />, root);
};

const toggle = () =>
	page.getByRole("button", { name: "Switch between light and dark" }).click();

const html = document.documentElement;

beforeEach(() => {
	localStorage.clear();
	html.removeAttribute(DATA_THEME);
});

afterEach(() => {
	dispose?.();
	document.body.replaceChildren();
});

test("switches a dark page to light and stores it where the classic console reads it", async () => {
	html.setAttribute(DATA_THEME, "dark");
	mountToggle();
	expect(getComputedStyle(html).colorScheme).toBe("dark");

	await toggle();

	expect(html.getAttribute(DATA_THEME)).toBe("light");
	expect(localStorage.getItem(BENCHER_THEME_KEY)).toBe("light");
	expect(getComputedStyle(html).colorScheme).toBe("light");
});

test("switches a light page back to dark", async () => {
	html.setAttribute(DATA_THEME, "light");
	mountToggle();

	await toggle();

	expect(html.getAttribute(DATA_THEME)).toBe("dark");
	expect(localStorage.getItem(BENCHER_THEME_KEY)).toBe("dark");
	expect(getComputedStyle(html).colorScheme).toBe("dark");
});

test("treats a page with no theme set as dark", async () => {
	mountToggle();
	expect(getComputedStyle(html).colorScheme).toBe("dark");

	await toggle();

	expect(html.getAttribute(DATA_THEME)).toBe("light");
});
