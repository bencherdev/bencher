import "@bencherdev/ui/styles.css";
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
const pinned = (canPin: boolean) => {
	const root = document.createElement("div");
	document.body.append(root);
	dispose = render(
		() => (
			<Header
				pinned="hash size=1, Latency on main"
				plotsHref="/plots"
				dirty={true}
				source={undefined}
				ready={true}
				canPin={canPin}
				hasLines={true}
				working={false}
				shared={false}
				confirmation={undefined}
				onShare={() => {}}
				onPin={() => {}}
				onUndo={() => {}}
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
