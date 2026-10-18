import "@bencherdev/ui/styles.css";
import { createSignal } from "solid-js";
import { render } from "solid-js/web";
import { afterEach, expect, test } from "vitest";
import { page, userEvent } from "vitest/browser";
import Share, { type Copied } from "./Share";

let dispose: (() => void) | undefined;
afterEach(() => {
	dispose?.();
	dispose = undefined;
	document.body.replaceChildren();
});

/** Share as a page holds it: each copy is recorded, and its confirmation shown. */
const mount = (embeddable: boolean) => {
	const copies: Copied[] = [];
	const root = document.createElement("div");
	document.body.append(root);
	dispose = render(() => {
		const [copied, setCopied] = createSignal<Copied>();
		return (
			<Share
				embeddable={embeddable}
				copied={copied()}
				onCopy={(what) => {
					copies.push(what);
					setCopied(what);
				}}
			/>
		);
	}, root);
	return copies;
};

const menu = () => page.getByRole("menu", { name: "Share" });

// Kills a public project's Share that copies at once, a menu the keyboard
// cannot open or walk, an item that copies the other thing, a confirmation
// that names the wrong copy, and focus left in a closed menu.
test("on a public project Share opens a menu of the link and the embed code", async () => {
	const copies = mount(true);
	const share = page.getByRole("button", { name: "Share" });
	await expect.element(share).toHaveAttribute("aria-haspopup", "menu");
	await expect.element(share).toHaveAttribute("aria-expanded", "false");
	(share.element() as HTMLElement).focus();
	await userEvent.keyboard("{Enter}");
	await expect.element(menu()).toBeVisible();
	await expect.element(share).toHaveAttribute("aria-expanded", "true");
	expect(
		menu()
			.getByRole("menuitem")
			.elements()
			.map((item) => item.textContent),
	).toEqual(["Copy link", "Copy embed code"]);
	await expect
		.element(page.getByRole("menuitem", { name: "Copy link" }))
		.toHaveFocus();
	expect(copies).toEqual([]);

	await userEvent.keyboard("{ArrowDown}");
	await expect
		.element(page.getByRole("menuitem", { name: "Copy embed code" }))
		.toHaveFocus();
	await userEvent.keyboard("{Enter}");
	expect(copies).toEqual(["embed"]);
	await expect.element(menu()).not.toBeInTheDocument();
	const copied = page.getByRole("button", { name: "Embed code copied" });
	await expect.element(copied).toHaveFocus();

	await userEvent.click(copied);
	await userEvent.click(page.getByRole("menuitem", { name: "Copy link" }));
	expect(copies).toEqual(["embed", "link"]);
	await expect
		.element(page.getByRole("button", { name: "Link copied" }))
		.toBeVisible();
});

// Kills Escape that leaves the menu open or drops the focus to the page.
test("Escape closes the menu and returns to Share", async () => {
	const copies = mount(true);
	await userEvent.click(page.getByRole("button", { name: "Share" }));
	await expect.element(menu()).toBeVisible();
	await userEvent.keyboard("{Escape}");
	await expect.element(menu()).not.toBeInTheDocument();
	await expect
		.element(page.getByRole("button", { name: "Share" }))
		.toHaveFocus();
	expect(copies).toEqual([]);
});

// Kills a private project offered its embed, which no one outside it can
// load, or a Share that asks before it copies the link.
test("on a private project Share copies the link at once", async () => {
	const copies = mount(false);
	const share = page.getByRole("button", { name: "Share" });
	await expect.element(share).not.toHaveAttribute("aria-haspopup");
	await userEvent.click(share);
	expect(copies).toEqual(["link"]);
	expect(page.getByRole("menu").elements()).toHaveLength(0);
	await expect
		.element(page.getByRole("button", { name: "Link copied" }))
		.toBeVisible();
});
