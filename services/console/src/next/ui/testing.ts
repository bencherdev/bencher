import { expect } from "vitest";

/**
 * The text a node shows, read as voice control and axe's label-in-name check
 * read it: text nodes in order, each element's own text trimmed, so a space
 * inside a child's edge is lost and two parts with none between them run together.
 */
export const shownText = (node: Node): string =>
	node.nodeType === Node.TEXT_NODE
		? (node.nodeValue ?? "")
		: node.nodeName === "BR"
			? " "
			: [...node.childNodes]
					.map(shownText)
					.join("")
					.replace(/\s+/g, " ")
					.trim();

/** Asserts that the element's accessible name starts with the text it shows. */
export const expectNameStartsWithText = (element: Element | null) => {
	const text = element ? shownText(element) : "";
	const name = element?.getAttribute("aria-label") ?? text;
	expect(text).not.toBe("");
	expect(name.toLowerCase().slice(0, text.length)).toBe(text.toLowerCase());
};

/**
 * Asserts that a click on any drawn cell of `row` lands on its link to `href`,
 * a click just above or below the row does not, and a click on any other
 * control or hover target in it lands on that control or target.
 */
export const expectRowOpens = (row: Element, href: string) => {
	const controls = [
		...row.querySelectorAll("a, button, input, label, .rowlink-own"),
	].filter((control) => control.getAttribute("href") !== href);
	let cells = 0;
	for (const cell of row.querySelectorAll("td")) {
		if (
			cell.getClientRects().length > 0 &&
			!controls.some((control) => cell.contains(control))
		) {
			cells++;
			expect(hitAt(cell)?.closest("a")?.getAttribute("href")).toBe(href);
		}
	}
	expect(cells).toBeGreaterThan(1);
	const box = row.getBoundingClientRect();
	for (const y of [box.top - 2, box.bottom + 2]) {
		const outside = document.elementFromPoint(box.x + box.width / 2, y);
		expect(outside).not.toBeNull();
		expect(outside?.closest("a")?.getAttribute("href")).not.toBe(href);
	}
	for (const control of controls) {
		expect(control.contains(hitAt(control))).toBe(true);
	}
};

/** What a click in the middle of `element` lands on, as the page hit-tests it. */
const hitAt = (element: Element) => {
	const box = element.getBoundingClientRect();
	return document.elementFromPoint(
		box.x + box.width / 2,
		box.y + box.height / 2,
	);
};
