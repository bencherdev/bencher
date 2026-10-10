import "@bencherdev/ui/styles.css";
import "./styles/console.css";
import { render } from "solid-js/web";
import { afterEach, expect, test } from "vitest";
import { page } from "vitest/browser";
import ContentSkeleton from "./ContentSkeleton";
import { handOff } from "./handoff";
import Shell from "./shell/Shell";

const disposals: (() => void)[] = [];

/** What both the static page and the app draw before the page's data. */
const paint = (mount: HTMLElement) => {
	disposals.push(
		render(
			() => (
				<>
					<Shell
						slug="hashbrown"
						tab="reports"
						organization={{ name: "Pompeii LLC", uuid: "org-uuid" }}
						project="Hashbrown"
						alerts={1}
					/>
					<ContentSkeleton />
				</>
			),
			mount,
		),
	);
};

const staticPage = () => {
	const mount = document.createElement("div");
	mount.className = "console";
	document.body.append(mount);
	paint(mount);
	return mount;
};

afterEach(() => {
	for (const dispose of disposals.splice(0)) {
		dispose();
	}
	document.body.replaceChildren();
});

// Kills a hand-off that drops the reader's focus to the page, and one that
// finds the new element by its link alone (the bell links to Alerts too).
test("focus moves to the same control in the app", async () => {
	await page.viewport(390, 844);
	const mount = staticPage();
	const tab = page.getByRole("link", { name: "Alerts 1 active" });
	const before = tab.element();
	(before as HTMLElement).focus();

	handOff(mount, () => paint(mount));

	expect(tab.element()).not.toBe(before);
	await expect.element(tab).toHaveFocus();
});

// Kills a skeleton whose wait starts over when the app draws it again.
test("a skeleton already shown stays shown", () => {
	const mount = staticPage();
	const shown = (skeleton: Element | null) =>
		skeleton?.checkVisibility({ visibilityProperty: true }) ?? false;
	for (const animation of mount.getAnimations({ subtree: true })) {
		animation.startTime = (document.timeline.currentTime as number) - 500;
	}
	expect(shown(mount.querySelector(".ui-skeleton"))).toBe(true);

	handOff(mount, () => paint(mount));

	expect(shown(mount.querySelector(".ui-skeleton"))).toBe(true);
});
