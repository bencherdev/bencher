import "@bencherdev/ui/styles.css";
import Dialog from "@bencherdev/ui/Dialog";
import { Show, createSignal } from "solid-js";
import { render } from "solid-js/web";
import { afterEach, expect, test } from "vitest";
import { page, userEvent } from "vitest/browser";

let dispose: (() => void) | undefined;

afterEach(() => {
	dispose?.();
	dispose = undefined;
	document.body.replaceChildren();
});

const open = ({ dismissible = true } = {}) => {
	const opener = document.createElement("button");
	opener.textContent = "Open";
	document.body.append(opener);
	opener.focus();
	const [shown, setShown] = createSignal(true);
	let dismissed = 0;
	const root = document.createElement("div");
	document.body.append(root);
	dispose = render(
		() => (
			<Show when={shown()}>
				<Dialog
					aria-labelledby="title"
					onDismiss={
						dismissible
							? () => {
									dismissed += 1;
								}
							: undefined
					}
				>
					<h2 id="title">Rename</h2>
					<input aria-label="Name" autofocus />
				</Dialog>
			</Show>
		),
		root,
	);
	return {
		opener,
		close: () => setShown(false),
		dismissed: () => dismissed,
		dialog: () => root.querySelector("dialog"),
	};
};

// Kills a dialog that is not modal (the page behind stays live), and one that
// leaves focus behind it.
test("opens modal, with focus inside it", async () => {
	const { dialog } = open();
	expect(dialog()?.matches(":modal")).toBe(true);
	await expect
		.element(page.getByRole("textbox", { name: "Name" }))
		.toHaveFocus();
	await expect
		.element(page.getByRole("dialog", { name: "Rename" }))
		.toBeVisible();
});

// Kills Escape closing the dialog behind its owner's back, which leaves the
// owner thinking it is open and unable to open it again.
test("Escape asks its owner to close it", async () => {
	const { dialog, dismissed } = open();
	await userEvent.keyboard("{Escape}");
	expect(dismissed()).toBe(1);
	expect(dialog()?.open).toBe(true);
});

// Kills a dialog removed without closing, which drops focus to the page.
test("closing returns focus to what opened it, and asks no one", async () => {
	const { opener, close, dismissed } = open();
	close();
	expect(document.activeElement).toBe(opener);
	await new Promise((resolve) => setTimeout(resolve, 0));
	expect(dismissed()).toBe(0);
});

// Kills a dialog the browser closed on its own, after repeated Escapes, that
// its owner still thinks is open and so can never open again.
test("a close the browser makes asks its owner to close it too", async () => {
	const { dialog, dismissed } = open();
	dialog()?.close();
	await expect.poll(dismissed).toBe(1);
});

// Kills Escape dismissing a dialog that must be answered, including the
// second Escape in a row, which the browser lets close any dialog.
test("without onDismiss, even repeated Escapes leave it open", async () => {
	const { dialog } = open({ dismissible: false });
	await userEvent.keyboard("{Escape}");
	expect(dialog()?.open).toBe(true);
	await userEvent.keyboard("{Escape}");
	await new Promise((resolve) => setTimeout(resolve, 50));
	expect(dialog()?.open).toBe(true);
	expect(dialog()?.matches(":modal")).toBe(true);
});
