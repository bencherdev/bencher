import "@bencherdev/ui/styles.css";
import Button from "@bencherdev/ui/Button";
import Icon from "@bencherdev/ui/Icon";
import { render } from "solid-js/web";
import { afterEach, expect, test } from "vitest";

let dispose: (() => void) | undefined;

afterEach(() => {
	dispose?.();
	dispose = undefined;
	document.body.replaceChildren();
});

// Kills a square rule that matches a button holding text beside its icon.
test("a button with text and an icon keeps its padding, one that is only an icon is square", () => {
	const root = document.createElement("div");
	document.body.append(root);
	dispose = render(
		() => (
			<>
				<Button>Show query</Button>
				<Button>
					Show query
					<Icon name="chevron-down" />
				</Button>
				<Button iconOnly aria-label="Close">
					<Icon name="close" />
				</Button>
			</>
		),
		root,
	);
	const [text, both, icon] = root.querySelectorAll<HTMLElement>(".ui-button");
	const padding = (button: HTMLElement | undefined) =>
		getComputedStyle(button as HTMLElement).paddingLeft;
	expect(padding(text)).not.toBe("0px");
	expect(padding(both)).toBe(padding(text));
	expect(padding(icon)).toBe("0px");
	const { width, height } = (icon as HTMLElement).getBoundingClientRect();
	expect(width).toBe(height);
});
