import "@bencherdev/ui/styles.css";
import "./explore.css";
import { createSignal } from "solid-js";
import { render } from "solid-js/web";
import { afterEach, expect, test } from "vitest";
import { page, userEvent } from "vitest/browser";
import Box, { type BoxValue, choicesOf } from "./Box";

let dispose: (() => void) | undefined;
afterEach(() => {
	dispose?.();
	dispose = undefined;
	document.body.replaceChildren();
});

// Kills a value removed by keyboard taking focus with it to the page, so the
// reader starts again from the top.
test("removing a value by keyboard keeps focus in the box", async () => {
	const root = document.createElement("div");
	document.body.append(root);
	dispose = render(() => {
		const [values, setValues] = createSignal([
			{ label: "main" },
			{ label: "feature" },
		]);
		return (
			<Box
				title="Branches"
				noun="branch"
				values={values()}
				onRemove={(index) =>
					setValues((all) => all.filter((_, at) => at !== index))
				}
				options={() => () => ({ options: [], more: 0 })}
				onAdd={() => {}}
			/>
		);
	}, root);

	(
		page
			.getByRole("button", { name: "Remove branch main" })
			.element() as HTMLElement
	).focus();
	await userEvent.keyboard("{Enter}");
	await expect
		.element(page.getByRole("button", { name: "Remove branch feature" }))
		.toHaveFocus();
	await userEvent.keyboard("{Enter}");
	await expect
		.element(page.getByRole("button", { name: "Add branch" }))
		.toHaveFocus();
});

// Kills units drawn beside the name, which crunches both, and a units line
// that only appears once the units arrive, which moves every box under it.
test("a two-line value puts its units under its name and keeps its height until they arrive", async () => {
	// Wide, where a value's own lines set its height rather than a phone's 44 px target.
	await page.viewport(1280, 800);
	const root = document.createElement("div");
	root.style.width = "180px";
	document.body.append(root);
	const [values, setValues] = createSignal<BoxValue[]>([
		{ label: "00000000-0000-4000-8000-000000000007" },
	]);
	dispose = render(
		() => (
			<Box
				title="Measures"
				noun="measure"
				values={values()}
				twoLine
				onRemove={() => {}}
				options={() => () => ({ options: [], more: 0 })}
				onAdd={() => {}}
			/>
		),
		root,
	);
	const value = () => root.querySelector(".ex-val") as HTMLElement;
	const before = value().getBoundingClientRect().height;

	setValues([{ label: "Latency", detail: "nanoseconds (ns)" }]);
	const name = page.getByText("Latency", { exact: true }).element();
	const units = page.getByText("nanoseconds (ns)", { exact: true }).element();
	expect(units.getBoundingClientRect().top).toBeGreaterThanOrEqual(
		name.getBoundingClientRect().bottom,
	);
	expect(value().getBoundingClientRect().height).toBe(before);
});

// Kills an add control that vanishes at the cap with no word of why, one a
// press still opens, and a count that names no box.
test("at the cap, the add control stays in place, disabled, and counts", async () => {
	const root = document.createElement("div");
	document.body.append(root);
	let opened = 0;
	dispose = render(
		() => (
			<Box
				title="Branches"
				noun="branch"
				values={Array.from({ length: 8 }, (_, n) => ({ label: `b${n}` }))}
				full
				onRemove={() => {}}
				options={() => () => ({ options: [], more: 0 })}
				onOpen={() => {
					opened += 1;
				}}
				onAdd={() => {}}
			/>
		),
		root,
	);
	const add = page.getByRole("button", { name: "Add branch, 8 of 8" });
	await expect.element(add).toHaveAttribute("aria-disabled", "true");
	await expect.element(add).toHaveTextContent("8 of 8");
	(add.element() as HTMLElement).click();
	expect(page.getByRole("menu").elements()).toHaveLength(0);
	expect(opened).toBe(0);
});

// Kills a menu that draws every choice it is handed, and one that stops at
// its first choices with no word of the rest.
test("a menu shows its first choices and counts the rest", async () => {
	const root = document.createElement("div");
	document.body.append(root);
	dispose = render(
		() => (
			<Box
				title="Parameters"
				noun="set"
				values={[]}
				onRemove={() => {}}
				options={() => () =>
					choicesOf(
						Array.from({ length: 16_000 }, (_, n) => ({
							value: String(n),
							label: `threads=${n}`,
						})),
					)
				}
				onAdd={() => {}}
			/>
		),
		root,
	);
	await page.getByRole("button", { name: "Add set" }).click();
	const menu = page.getByRole("menu", { name: "Add set" });
	await expect
		.element(menu.getByRole("menuitem", { name: "15,968 more" }))
		.toBeDisabled();
	expect(menu.getByRole("menuitem").elements()).toHaveLength(33);
});
