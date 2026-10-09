import "@bencherdev/ui/styles.css";
import "../styles/console.css";
import Button from "@bencherdev/ui/Button";
import ButtonLink from "@bencherdev/ui/ButtonLink";
import { render } from "solid-js/web";
import { afterEach, expect, test } from "vitest";
import { page, userEvent } from "vitest/browser";

let dispose: (() => void) | undefined;

afterEach(() => {
	dispose?.();
	dispose = undefined;
	document.body.replaceChildren();
});

const LOOK = [
	"backgroundColor",
	"borderTopColor",
	"color",
	"height",
	"paddingLeft",
	"borderTopLeftRadius",
	"fontSize",
] as const;

const look = (element: Element) => {
	const style = getComputedStyle(element);
	return Object.fromEntries(LOOK.map((key) => [key, style[key]]));
};

// Kills a link form that does not navigate, one that drops the Button's
// variant or size, and a console link rule that recolors it on hover.
test("a ButtonLink is a link drawn as the Button of its variant and size, hovered too", async () => {
	const root = document.createElement("div");
	root.className = "console";
	document.body.append(root);
	dispose = render(
		() => (
			<>
				<Button variant="primary" size="sm">
					Act
				</Button>
				<ButtonLink href="/next/" variant="primary" size="sm">
					Go
				</ButtonLink>
				<ButtonLink href="/next/">Plain</ButtonLink>
				<Button>Plain act</Button>
			</>
		),
		root,
	);
	const link = page.getByRole("link", { name: "Go" });
	await expect.element(link).toHaveAttribute("href", "/next/");
	const button = page.getByRole("button", { name: "Act", exact: true });
	expect(look(link.element())).toEqual(look(button.element()));
	expect(look(page.getByRole("link", { name: "Plain" }).element())).toEqual(
		look(page.getByRole("button", { name: "Plain act" }).element()),
	);
	expect(look(link.element())).not.toEqual(
		look(page.getByRole("link", { name: "Plain" }).element()),
	);
	await userEvent.hover(link);
	expect(getComputedStyle(link.element()).color).toBe(
		getComputedStyle(button.element()).color,
	);
});
