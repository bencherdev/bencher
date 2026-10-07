import { batch, createSignal } from "solid-js";
import { render } from "solid-js/web";
import { afterEach, expect, test } from "vitest";
import { page } from "vitest/browser";
import Announcer from "./Announcer";

let dispose: (() => void) | undefined;

afterEach(() => {
	dispose?.();
	document.body.replaceChildren();
});

// Kills a first load announced over the browser's own reading of the title,
// a title change without a navigation announced, and a navigation that says nothing.
test("says the new page's title on each navigation, and only then", async () => {
	const [path, setPath] = createSignal("/hashbrown/reports");
	const [title, setTitle] = createSignal("Reports | hashbrown | Bencher");
	const root = document.createElement("div");
	document.body.append(root);
	dispose = render(() => <Announcer path={path()} title={title()} />, root);
	const status = page.getByRole("status");

	await expect.element(status).toBeEmptyDOMElement();

	setTitle("Reports | Hashbrown | Bencher");
	await expect.element(status).toBeEmptyDOMElement();

	batch(() => {
		setPath("/hashbrown/plots");
		setTitle("Plots | Hashbrown | Bencher");
	});
	await expect.element(status).toHaveTextContent("Plots | Hashbrown | Bencher");
});
