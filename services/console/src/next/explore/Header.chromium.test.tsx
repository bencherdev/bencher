import "@bencherdev/ui/styles.css";
import "../styles/console.css";
import "./explore.css";
import { render } from "solid-js/web";
import { afterEach, expect, test } from "vitest";
import { page } from "vitest/browser";
import Header from "./Header";

let dispose: (() => void) | undefined;
afterEach(() => {
	dispose?.();
	dispose = undefined;
	document.body.replaceChildren();
});

/** A pinned plot with unsaved changes, for a reader who may or may not pin. */
const pinned = (canPin: boolean, title = "hash size=1, Latency on main") => {
	const root = document.createElement("div");
	document.body.append(root);
	dispose = render(
		() => (
			<Header
				pinned={title}
				plotsHref="/plots"
				dirty={true}
				source={undefined}
				ready={true}
				canPin={canPin}
				full={false}
				hasLines={true}
				working={false}
				embeddable={true}
				copied={undefined}
				confirmation={undefined}
				onCopy={() => {}}
				onPin={() => {}}
				onUndo={() => {}}
				onEdit={() => {}}
				onDiscard={() => {}}
				onSave={() => {}}
				onSaveNew={() => {}}
			/>
		),
		root,
	);
};

const actions = () =>
	page
		.getByRole("button")
		.elements()
		.map((button) => button.textContent?.trim());

// Kills Discard, Save, or Save as new drawn on a pin for a reader who cannot
// change it, and a pin whose editor cannot save it.
test("on a pinned plot, a reader who cannot pin may only share", async () => {
	pinned(false);
	await expect
		.element(page.getByRole("button", { name: "Share" }))
		.toBeVisible();
	expect(actions()).toEqual(["Share"]);

	dispose?.();
	pinned(true);
	await expect
		.element(page.getByRole("button", { name: "Save", exact: true }))
		.toBeVisible();
	expect(actions()).toEqual(["Discard", "Save as new", "Save", "Share"]);
});

// Kills a pin title with no break that pushes the page sideways on a phone,
// wraps to lines, or cuts with no way to read the rest.
test("a long pin title keeps one line on a phone and opens in place", async () => {
	await page.viewport(320, 700);
	pinned(true, "a".repeat(64));
	const title = page.getByRole("heading", { level: 1 });
	await expect.element(title).toBeVisible();
	expect(document.documentElement.scrollWidth).toBe(320);
	const line = title.element().getBoundingClientRect().height;
	const reveal = title.getByRole("button", { name: "a".repeat(64) });
	await reveal.click();
	await expect.element(reveal).toHaveAttribute("aria-expanded", "true");
	expect(title.element().getBoundingClientRect().height).toBeGreaterThan(line);
	expect(document.documentElement.scrollWidth).toBe(320);
});
