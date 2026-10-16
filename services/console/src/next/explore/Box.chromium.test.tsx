import "@bencherdev/ui/styles.css";
import "./explore.css";
import { createSignal } from "solid-js";
import { render } from "solid-js/web";
import { afterEach, expect, test } from "vitest";
import { page, userEvent } from "vitest/browser";
import Box, { type BoxValue } from "./Box";

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
				options={() => () => []}
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
				options={() => () => []}
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
