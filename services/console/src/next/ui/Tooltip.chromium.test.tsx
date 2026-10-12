import "@bencherdev/ui/styles.css";
import Tooltip from "@bencherdev/ui/Tooltip";
import { createSignal, type JSX, Show } from "solid-js";
import { render } from "solid-js/web";
import { afterEach, beforeEach, expect, test } from "vitest";
import { page, userEvent } from "vitest/browser";

let dispose: (() => void) | undefined;

beforeEach(async () => {
	await page.viewport(1280, 800);
});

afterEach(() => {
	dispose?.();
	dispose = undefined;
	document.body.replaceChildren();
});

// Longer than the moment the tooltip waits for the pointer to reach it.
const pastGrace = () => new Promise((resolve) => setTimeout(resolve, 400));

const mount = (
	style = "",
	text: () => JSX.Element = () => "Hides the line",
) => {
	const root = document.createElement("div");
	document.body.append(root);
	dispose = render(
		() => (
			<>
				<Tooltip tip={text()}>
					<button type="button" style={style}>
						sha256
					</button>
				</Tooltip>
				<button type="button" style="margin-top: 80px">
					Next
				</button>
			</>
		),
		root,
	);
	const tip = root.querySelector<HTMLElement>(".ui-tooltip");
	if (!tip) {
		throw new Error("No tooltip");
	}
	return {
		button: page.getByRole("button", { name: "sha256" }),
		next: page.getByRole("button", { name: "Next" }),
		tip,
		open: () => tip.matches(":popover-open"),
	};
};

// Kills a tooltip shown after a delay or never, one no screen reader hears, one gone before the pointer can reach it, and one left up once the pointer has gone.
test("shows at once on hover, describes its control, and hides a moment after the pointer leaves", async () => {
	const { button, next, open } = mount();
	await expect.element(button).toHaveAccessibleDescription("Hides the line");
	expect(open()).toBe(false);

	await button.hover();
	expect(open()).toBe(true);

	await next.hover();
	expect(open()).toBe(true);
	await expect.poll(open).toBe(false);
});

// Kills a tooltip the pointer cannot move onto, or one that hides while the pointer is on it.
test("stays while the pointer is on the tooltip", async () => {
	const { button, next, tip, open } = mount();
	await button.hover();
	await page.elementLocator(tip).hover();
	await pastGrace();
	expect(open()).toBe(true);

	await next.hover();
	await expect.poll(open).toBe(false);
});

// Kills an Escape that does nothing, one heard only while its control holds the focus, or a tooltip that comes back under a still pointer.
test("hides on Escape without moving the focus or the pointer", async () => {
	const { button, open } = mount();
	await button.hover();
	const focus = document.activeElement;
	await userEvent.keyboard("{Escape}");
	expect(open()).toBe(false);
	expect(document.activeElement).toBe(focus);
	await pastGrace();
	expect(open()).toBe(false);

	await userEvent.tab();
	expect(document.activeElement).toBe(button.element());
	expect(open()).toBe(true);
	await userEvent.keyboard("{Escape}");
	expect(open()).toBe(false);
	expect(document.activeElement).toBe(button.element());
});

// Kills a tooltip the keyboard never sees, one that leaves while its control keeps the focus, and one that outlives the focus.
test("shows on keyboard focus and stays until the focus leaves", async () => {
	const { button, next, open } = mount();
	// The pointer rests off the control, so only the focus holds the tooltip.
	await next.hover();
	await userEvent.tab();
	expect(document.activeElement).toBe(button.element());
	expect(open()).toBe(true);
	await pastGrace();
	expect(open()).toBe(true);

	await userEvent.tab();
	await expect.poll(open).toBe(false);
});

// Kills a press that hides the tooltip of a control the pointer still holds, or the focus a press gives taken for the keyboard's.
test("stays through a press on its control, and leaves with the pointer", async () => {
	const { button, next, open } = mount();
	await button.click();
	expect(document.activeElement).toBe(button.element());
	expect(open()).toBe(true);
	await pastGrace();
	expect(open()).toBe(true);

	await next.hover();
	await expect.poll(open).toBe(false);
});

// Kills a tooltip a finger opens, which a phone has no way to dismiss by moving away.
test("never shows for a touch", () => {
	const { button, open } = mount();
	button
		.element()
		.dispatchEvent(new PointerEvent("pointerenter", { pointerType: "touch" }));
	expect(open()).toBe(false);
});

// Kills a phone layout that still shows tooltips, which every control there must read without.
test("never shows on a phone", async () => {
	await page.viewport(390, 844);
	const { button, open } = mount();
	await button.hover();
	expect(open()).toBe(false);
});

// Kills a tooltip centered without regard to the screen, or above a control at the top edge.
test("stays inside the screen beside a control at any edge", async () => {
	for (const corner of [
		"top: 0; left: 0",
		"top: 0; right: 0",
		"bottom: 0; left: 0",
		"bottom: 0; right: 0",
	]) {
		const { button, tip, open } = mount(`position: fixed; ${corner}`);
		await button.hover();
		expect(open(), corner).toBe(true);
		const box = tip.getBoundingClientRect();
		const control = button.element().getBoundingClientRect();
		expect(box.left, corner).toBeGreaterThanOrEqual(0);
		expect(box.top, corner).toBeGreaterThanOrEqual(0);
		expect(box.right, corner).toBeLessThanOrEqual(
			document.documentElement.clientWidth,
		);
		expect(box.bottom, corner).toBeLessThanOrEqual(innerHeight);
		// Beside the control, never over it.
		expect(box.bottom <= control.top || box.top >= control.bottom, corner).toBe(
			true,
		);
		dispose?.();
		dispose = undefined;
		document.body.replaceChildren();
	}
});

// Kills a tooltip placed only for its first text, which grows over its control once the text grows, or squeezes into the room its first place left.
test("places itself again when its text changes while it shows", async () => {
	const [long, setLong] = createSignal(false);
	const { button, tip, open } = mount(
		"position: fixed; bottom: 0; right: 0",
		() =>
			long()
				? "Shows the line again, with every point it had in the window"
				: "Hides",
	);
	await button.hover();
	expect(open()).toBe(true);
	const short = tip.getBoundingClientRect().width;
	setLong(true);
	const control = button.element().getBoundingClientRect();
	await expect
		.poll(() => tip.getBoundingClientRect().height)
		.toBeGreaterThan(30);
	expect(tip.getBoundingClientRect().bottom).toBeLessThanOrEqual(control.top);
	expect(tip.getBoundingClientRect().width).toBeGreaterThan(3 * short);
	expect(tip.getBoundingClientRect().right).toBeLessThanOrEqual(
		document.documentElement.clientWidth,
	);
	expect(open()).toBe(true);
});

const beside = (tip: HTMLElement, control: Element) => {
	const box = tip.getBoundingClientRect();
	const at = control.getBoundingClientRect();
	return (
		box.top >= 0 &&
		box.bottom <= innerHeight &&
		(box.bottom <= at.top || box.top >= at.bottom) &&
		box.bottom >= at.top - 40 &&
		box.top <= at.bottom + 40
	);
};

// Kills a tooltip placed before the focus scrolls its control into view, or placed once and left behind by a scroll.
test("stays beside a control the focus scrolls into view, and through a scroll", async () => {
	const { button, tip, open } = mount("margin-top: 1600px");
	await userEvent.tab();
	expect(document.activeElement).toBe(button.element());
	expect(open()).toBe(true);
	await expect.poll(() => beside(tip, button.element())).toBe(true);

	window.scrollBy({ top: 100, behavior: "instant" });
	await expect.poll(() => beside(tip, button.element())).toBe(true);
	expect(open()).toBe(true);
	window.scrollTo({ top: 0, behavior: "instant" });
});

// Kills a tooltip left behind when the window's size moves its control.
test("stays beside its control through a resize", async () => {
	const { button, tip, open } = mount("position: fixed; top: 200px; right: 0");
	await button.hover();
	expect(beside(tip, button.element())).toBe(true);
	await page.viewport(1000, 800);
	await expect.poll(() => beside(tip, button.element())).toBe(true);
	expect(tip.getBoundingClientRect().right).toBeLessThanOrEqual(
		document.documentElement.clientWidth,
	);
	expect(open()).toBe(true);
});

// Kills a tooltip that leaves with the pointer while its control keeps the keyboard focus.
test("stays while its control keeps the keyboard focus, once the pointer leaves", async () => {
	const { button, next, open } = mount();
	await next.hover();
	await userEvent.tab();
	await button.hover();
	await next.hover();
	await pastGrace();
	expect(open()).toBe(true);
});

// Kills a tooltip that leaves with the focus while the pointer is still on its control.
test("stays while the pointer is on its control, once the focus leaves", async () => {
	const { button, open } = mount();
	await button.hover();
	await userEvent.tab();
	expect(document.activeElement).toBe(button.element());
	await userEvent.tab();
	await pastGrace();
	expect(open()).toBe(true);
});

// Kills a tooltip any key hides, such as the one that presses its control, or one left where its old text sat.
test("stays through a keyboard press that changes its text, beside its control", async () => {
	const [hidden, setHidden] = createSignal(false);
	const root = document.createElement("div");
	root.style.cssText = "position: fixed; bottom: 0; right: 0";
	document.body.append(root);
	dispose = render(
		() => (
			<Tooltip
				tip={hidden() ? "Show the line, with every point it had" : "Hide"}
			>
				<button type="button" onClick={() => setHidden(!hidden())}>
					sha256
				</button>
			</Tooltip>
		),
		root,
	);
	const tip = root.querySelector<HTMLElement>(".ui-tooltip") as HTMLElement;
	const button = page.getByRole("button", { name: "sha256" });
	await userEvent.tab();
	expect(tip.matches(":popover-open")).toBe(true);
	await userEvent.keyboard("{Enter}");
	expect(hidden()).toBe(true);
	expect(tip.matches(":popover-open")).toBe(true);
	expect(beside(tip, button.element())).toBe(true);
});

// Kills a tooltip that drops the description its control already had, or takes it along when it goes.
test("adds its description to the control's own, and takes back only its own", async () => {
	const [wrapped, setWrapped] = createSignal(true);
	const root = document.createElement("div");
	document.body.append(root);
	const input = (
		<input aria-label="Seconds" aria-describedby="hint" />
	) as HTMLInputElement;
	dispose = render(
		() => (
			<>
				<span id="hint">Whole numbers only</span>
				<Show when={wrapped()} fallback={input}>
					<Tooltip tip="Time to wait">{input}</Tooltip>
				</Show>
			</>
		),
		root,
	);
	const field = page.getByRole("textbox", { name: "Seconds" });
	await expect
		.element(field)
		.toHaveAccessibleDescription("Whole numbers only Time to wait");
	setWrapped(false);
	expect(input.getAttribute("aria-describedby")).toBe("hint");
});
