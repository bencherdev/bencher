import "@bencherdev/ui/styles.css";
import "./explore.css";
import { createSignal } from "solid-js";
import { render } from "solid-js/web";
import { afterEach, expect, test } from "vitest";
import { page, userEvent } from "vitest/browser";
import Box from "./Box";

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
