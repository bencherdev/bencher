import "@bencherdev/ui/styles.css";
import Menu, { MenuItemRadio } from "@bencherdev/ui/Menu";
import { For, Show, createSignal } from "solid-js";
import { render } from "solid-js/web";
import { afterEach, expect, test } from "vitest";
import { page, userEvent } from "vitest/browser";

const ITEMS = ["main", "feature-simd", "release"];

let dispose: (() => void) | undefined;
const picked: string[] = [];

const mount = ({
	search = false,
	more,
}: {
	search?: boolean;
	more?: number;
} = {}) => {
	picked.length = 0;
	const root = document.createElement("div");
	document.body.append(root);
	dispose = render(() => {
		const [open, setOpen] = createSignal(false);
		const [value, setValue] = createSignal("feature-simd");
		let anchor: HTMLButtonElement | undefined;
		return (
			<>
				<span style={{ position: "relative" }}>
					<button
						ref={anchor}
						type="button"
						aria-haspopup="menu"
						aria-expanded={open()}
						onClick={() => setOpen(!open())}
					>
						Branch
					</button>
					<Show when={open()}>
						<Menu
							label="Branch"
							anchor={anchor}
							onClose={() => setOpen(false)}
							more={more}
							header={
								search ? (
									<input type="search" aria-label="Search branches" />
								) : undefined
							}
						>
							<For each={ITEMS}>
								{(item) => (
									<MenuItemRadio
										checked={item === value()}
										onSelect={() => {
											picked.push(item);
											setValue(item);
											setOpen(false);
										}}
									>
										{item}
									</MenuItemRadio>
								)}
							</For>
						</Menu>
					</Show>
				</span>
				<button type="button">Elsewhere</button>
			</>
		);
	}, root);
};

const trigger = () => page.getByRole("button", { name: "Branch" });
const item = (name: string) => page.getByRole("menuitemradio", { name });

afterEach(() => {
	dispose?.();
	document.body.replaceChildren();
});

// Kills a menu that opens without the focus, or on its first item rather than the current one.
test("opening focuses the checked item", async () => {
	mount();
	await trigger().click();
	await expect
		.element(page.getByRole("menu", { name: "Branch" }))
		.toBeVisible();
	await expect.element(item("feature-simd")).toHaveFocus();
	await expect.element(item("feature-simd")).toBeChecked();
});

// Kills arrows that stop at the ends, Home and End that do nothing, and a
// choice by keyboard that leaves the focus on nothing.
test("the arrow keys wrap, Enter chooses, and the focus returns to the trigger", async () => {
	mount();
	await trigger().click();

	await userEvent.keyboard("{ArrowDown}");
	await expect.element(item("release")).toHaveFocus();
	await userEvent.keyboard("{ArrowDown}");
	await expect.element(item("main")).toHaveFocus();
	await userEvent.keyboard("{ArrowUp}");
	await expect.element(item("release")).toHaveFocus();
	await userEvent.keyboard("{Home}");
	await expect.element(item("main")).toHaveFocus();
	await userEvent.keyboard("{End}");
	await expect.element(item("release")).toHaveFocus();
	await userEvent.keyboard("{Enter}");

	expect(picked).toEqual(["release"]);
	await expect.element(page.getByRole("menu")).not.toBeInTheDocument();
	await expect.element(trigger()).toHaveFocus();
});

// Kills a menu that stops with no word of the rest, a count without its
// thousands grouped, and a count the keys can land on or choose.
test("a menu that holds more says how many, in an item no key reaches", async () => {
	mount({ more: 1234 });
	await trigger().click();
	const rest = page.getByRole("menuitem", { name: "1,234 more" });
	await expect.element(rest).toBeDisabled();
	await userEvent.keyboard("{End}");
	await expect.element(item("release")).toHaveFocus();
	await userEvent.keyboard("{ArrowDown}");
	await expect.element(item("main")).toHaveFocus();
});

// Kills a "0 more" left in a menu that shows every choice.
test("a menu that shows every choice says nothing more", async () => {
	mount({ more: 0 });
	await trigger().click();
	await expect.element(item("main")).toBeVisible();
	expect(page.getByRole("menuitem").elements()).toHaveLength(0);
});

// Kills a menu Escape cannot close, or that drops the focus when it closes.
test("Escape closes the menu and returns the focus", async () => {
	mount();
	await trigger().click();
	await userEvent.keyboard("{Escape}");

	await expect.element(page.getByRole("menu")).not.toBeInTheDocument();
	await expect.element(trigger()).toHaveFocus();
	expect(picked).toEqual([]);
});

// Kills a menu Tab leaves open behind the focus as it moves on.
test("Tab closes the menu and moves on", async () => {
	mount();
	await trigger().click();
	await userEvent.keyboard("{Tab}");

	await expect.element(page.getByRole("menu")).not.toBeInTheDocument();
	expect(picked).toEqual([]);
});

// Kills a menu that stays open over the page, and one its own trigger cannot close.
test("a press outside closes it, and the trigger toggles it", async () => {
	mount();
	await trigger().click();
	await page.getByRole("button", { name: "Elsewhere" }).click();
	await expect.element(page.getByRole("menu")).not.toBeInTheDocument();

	await trigger().click();
	await expect.element(page.getByRole("menu")).toBeVisible();
	await trigger().click();
	await expect.element(page.getByRole("menu")).not.toBeInTheDocument();
});

// Kills a search box that does not get the focus, and arrows that cannot leave it.
test("with a search box, it takes the focus and the arrows step into the items", async () => {
	mount({ search: true });
	await trigger().click();
	await expect
		.element(page.getByRole("searchbox", { name: "Search branches" }))
		.toHaveFocus();

	await userEvent.keyboard("{ArrowDown}");
	await expect.element(item("main")).toHaveFocus();
	await userEvent.keyboard("{ArrowUp}");
	await userEvent.keyboard("{ArrowUp}");
	await expect.element(item("feature-simd")).toHaveFocus();
});
