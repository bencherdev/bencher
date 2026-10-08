import "@bencherdev/ui/styles.css";
import Sheet from "@bencherdev/ui/Sheet";
import { createSignal } from "solid-js";
import { render } from "solid-js/web";
import { afterEach, expect, test } from "vitest";
import { page, userEvent } from "vitest/browser";

let dispose: (() => void) | undefined;

const mount = (side = false) => {
	const root = document.createElement("div");
	document.body.append(root);
	dispose = render(() => {
		const [open, setOpen] = createSignal(false);
		return (
			<>
				<button type="button" onClick={() => setOpen(true)}>
					Filters
				</button>
				<Sheet
					open={open()}
					onClose={() => setOpen(false)}
					title="Filters"
					side={side}
				>
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

// Kills a side sheet drawn across the foot of a wide screen, or docked on a phone.
test.each([
	[1280, { right: 1280, top: 0, height: 720 }],
	[390, { right: 390, top: undefined, height: undefined }],
] as const)(
	"a side sheet at %i px docks to the right only on a wide screen",
	async (width, expected) => {
		await page.viewport(width, 720);
		mount(true);
		await opener().click();
		const box = sheet().element().getBoundingClientRect();
		expect(box.right).toBe(expected.right);
		if (expected.top === undefined) {
			expect(box.left).toBe(0);
			expect(box.bottom).toBe(720);
		} else {
			expect(box.top).toBe(expected.top);
			expect(box.height).toBe(expected.height);
			expect(box.width).toBe(560);
		}
	},
);
