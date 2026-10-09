import "@bencherdev/ui/styles.css";
import Chip from "@bencherdev/ui/Chip";
import { render } from "solid-js/web";
import { afterEach, expect, test } from "vitest";
import { shownText } from "./testing";

let dispose: (() => void) | undefined;

afterEach(() => {
	dispose?.();
	dispose = undefined;
	document.body.replaceChildren();
});

// Kills a key run into its value ("branchmain"): voice control and the
// label-in-name check read the text as written, not as laid out.
test("a chip's key and value read as two words without moving", () => {
	const root = document.createElement("div");
	document.body.append(root);
	dispose = render(
		() => (
			<>
				<Chip label="branch" caret>
					main
				</Chip>
				<Chip label="branch" caret>
					<span>main</span>
				</Chip>
			</>
		),
		root,
	);
	const [text, wrapped] = root.querySelectorAll<HTMLElement>(".ui-chip");
	expect(text && shownText(text)).toBe("branch main");
	expect(wrapped && shownText(wrapped)).toBe("branch main");
	expect(text?.getBoundingClientRect().width).toBe(
		wrapped?.getBoundingClientRect().width,
	);
});
