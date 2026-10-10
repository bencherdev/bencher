import "@bencherdev/ui/styles.css";
import Sheet from "@bencherdev/ui/Sheet";
import { createSignal } from "solid-js";
import { render } from "solid-js/web";
import { afterEach, expect, test } from "vitest";
import { page, userEvent } from "vitest/browser";

let dispose: (() => void) | undefined;

const mount = () => {
	const root = document.createElement("div");
	document.body.append(root);
	dispose = render(() => {
		const [open, setOpen] = createSignal(false);
		return (
			<>
				<button type="button" onClick={() => setOpen(true)}>
					Filters
				</button>
				<Sheet open={open()} onClose={() => setOpen(false)} title="Filters">
					<button type="button">Filter by branch</button>
				</Sheet>
			</>
		);
	}, root);
};

const opener = () => page.getByRole("button", { name: "Filters" });
const sheet = () => page.getByRole("dialog", { name: "Filters" });
const isOpen = () => document.querySelector("dialog")?.open;

afterEach(() => {
	dispose?.();
	document.body.replaceChildren();
});

// Kills a sheet that does not open as a named modal, or leaves the page behind it reachable.
test("opens as a modal dialog named by its title", async () => {
	mount();
	await opener().click();

	await expect.element(sheet()).toBeVisible();
	expect((sheet().element() as HTMLDialogElement).matches(":modal")).toBe(true);
	const behind = document.querySelector("button") as HTMLButtonElement;
	behind.focus();
	expect(document.activeElement).not.toBe(behind);
});

// Kills a Done button that does not close it, and a sheet that drops the focus on close.
test("Done closes it and the focus returns to what opened it", async () => {
	mount();
	await opener().click();
	await sheet().getByRole("button", { name: "Done" }).click();

	await expect.poll(isOpen).toBe(false);
	await expect.element(opener()).toHaveFocus();
});

// Kills a sheet Escape or a press on the dimmed page cannot close, which the
// parent would then still believe open.
test("Escape and a press on the dimmed page close it", async () => {
	mount();
	await opener().click();
	await userEvent.keyboard("{Escape}");
	await expect.poll(isOpen).toBe(false);

	await opener().click();
	await expect.element(sheet()).toBeVisible();
	const dialog = sheet().element() as HTMLDialogElement;
	dialog.dispatchEvent(new MouseEvent("click", { bubbles: true }));
	await expect.poll(isOpen).toBe(false);
});
