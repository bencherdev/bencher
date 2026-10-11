import "@bencherdev/ui/styles.css";
import Card from "@bencherdev/ui/Card";
import { render } from "solid-js/web";
import { afterEach, expect, test } from "vitest";

let dispose: (() => void) | undefined;

afterEach(() => {
	dispose?.();
	dispose = undefined;
	document.body.replaceChildren();
});

/** The color a background token paints, as the browser computes it. */
const painted = (token: string) => {
	const probe = document.createElement("div");
	probe.style.background = `var(${token})`;
	document.body.append(probe);
	const color = getComputedStyle(probe).backgroundColor;
	probe.remove();
	return color;
};

// Kills a soft card drawn on the raised surface, as the default card is.
test("a soft card sits on the card ground, the default on the surface", () => {
	const root = document.createElement("div");
	document.body.append(root);
	dispose = render(
		() => (
			<>
				<Card>Default</Card>
				<Card variant="soft">Soft</Card>
			</>
		),
		root,
	);
	const [plain, soft] = root.querySelectorAll<HTMLElement>(".ui-card");
	expect(painted("--color-background-card")).not.toBe(
		painted("--color-background-surface"),
	);
	expect(getComputedStyle(soft as HTMLElement).backgroundColor).toBe(
		painted("--color-background-card"),
	);
	expect(getComputedStyle(plain as HTMLElement).backgroundColor).toBe(
		painted("--color-background-surface"),
	);
});
