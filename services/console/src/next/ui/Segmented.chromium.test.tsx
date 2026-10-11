import "@bencherdev/ui/styles.css";
import Segmented from "@bencherdev/ui/Segmented";
import { createSignal } from "solid-js";
import { render } from "solid-js/web";
import { afterEach, expect, test } from "vitest";
import { page, userEvent } from "vitest/browser";

const OPTIONS = [
	{ value: "1w", label: "1w" },
	{ value: "4w", label: "4w" },
	{ value: "all", label: "All" },
] as const;

let dispose: (() => void) | undefined;
const changes: string[] = [];

const mount = (initial: string | undefined) => {
	changes.length = 0;
	const root = document.createElement("div");
	document.body.append(root);
	dispose = render(() => {
		const [value, setValue] = createSignal(initial);
		return (
			<>
				<button type="button">Before</button>
				<Segmented
					name="window"
					aria-label="Window"
					options={OPTIONS}
					value={value()}
					onChange={(next) => {
						changes.push(next);
						setValue(next);
					}}
				/>
				<button type="button">After</button>
			</>
		);
	}, root);
	return root;
};

const radio = (name: string) => page.getByRole("radio", { name });

afterEach(() => {
	dispose?.();
	dispose = undefined;
	document.body.replaceChildren();
});

// Kills a group that checks nothing on click, or checks more than one.
test("a click checks one position", async () => {
	mount("4w");
	await expect.element(radio("4w")).toBeChecked();

	await radio("All").click();

	expect(changes).toEqual(["all"]);
	await expect.element(radio("All")).toBeChecked();
	await expect.element(radio("4w")).not.toBeChecked();
});

// Kills positions that are not one radio group (separate Tab stops a keyboard
// reader must pass one by one), or a group entered at its first position
// rather than the checked one.
test("Tab enters at the checked position and leaves the group in one step", async () => {
	mount("4w");
	(document.querySelector("button") as HTMLButtonElement).focus();

	await userEvent.keyboard("{Tab}");
	await expect.element(radio("4w")).toHaveFocus();
	await userEvent.keyboard("{Tab}");
	await expect
		.element(page.getByRole("button", { name: "After" }))
		.toHaveFocus();
});

// Kills arrows that move the focus without the choice, or that stop at the ends.
test("the arrow keys move the choice and wrap", async () => {
	mount("4w");
	(radio("4w").element() as HTMLElement).focus();

	await userEvent.keyboard("{ArrowRight}");
	await expect.element(radio("All")).toHaveFocus();
	await userEvent.keyboard("{ArrowRight}");
	await expect.element(radio("1w")).toHaveFocus();
	await userEvent.keyboard("{ArrowLeft}");

	expect(changes).toEqual(["all", "1w", "all"]);
	await expect.element(radio("All")).toBeChecked();
});

// Kills a group with no way in when nothing is checked, such as a custom window.
test("with nothing checked, Tab enters at the first position", async () => {
	mount(undefined);
	(document.querySelector("button") as HTMLButtonElement).focus();

	await userEvent.keyboard("{Tab}");
	await expect.element(radio("1w")).toHaveFocus();
	await expect.element(radio("1w")).not.toBeChecked();
});

// Kills positions under 44 px on a narrow screen.
test("positions are 44 px tall on a narrow screen", async () => {
	await page.viewport(390, 844);
	const root = mount("4w");
	const positions = root.querySelectorAll(".ui-segment");
	expect(positions.length).toBe(OPTIONS.length);
	for (const position of positions) {
		expect(position.getBoundingClientRect().height).toBeGreaterThanOrEqual(44);
	}
});
