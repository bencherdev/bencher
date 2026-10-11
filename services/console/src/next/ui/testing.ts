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
