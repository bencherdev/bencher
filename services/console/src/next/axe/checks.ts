import type { RunOptions } from "axe-core";

/** Off by default in axe; a control's name starts with its visible label. */
export const AXE_OPTIONS: RunOptions = {
	rules: { "label-content-name-mismatch": { enabled: true } },
};

/**
 * Each control whose accessible name does not start with the text it shows,
 * both read as words as axe reads them: its label-in-name rule only asks that
 * the name contain the text. Self-contained, as Playwright runs it in the page
 * from its source.
 */
export const misnamed = (): string[] => {
	const hidden = (element: Element) =>
		!element.checkVisibility({ visibilityProperty: true }) ||
		element.closest(".sr-only") !== null;
	const words = (text: string) =>
		text
			.replace(/[^\p{L}\p{N}]+/gu, " ")
			.trim()
			.toLowerCase();
	const shown = (node: Node): string =>
		node.nodeType === Node.TEXT_NODE
			? (node.nodeValue ?? "")
			: !(node instanceof Element) || hidden(node)
				? ""
				: node.nodeName === "BR"
					? " "
					: [...node.childNodes]
							.map(shown)
							.join("")
							.replace(/\s+/g, " ")
							.trim();
	const controls = document.querySelectorAll(
		'a[href], button, summary, [role="button"], [role="link"], [role="tab"], [role^="menuitem"], [role="checkbox"], [role="radio"], [role="switch"], [role="option"]',
	);
	return [...controls].flatMap((control) => {
		const labelledBy = control.getAttribute("aria-labelledby");
		const name = labelledBy
			? labelledBy
					.split(/\s+/)
					.map((id) => document.getElementById(id)?.textContent ?? "")
					.join(" ")
			: control.getAttribute("aria-label");
		const text = shown(control);
		if (name === null || hidden(control) || words(text) === "") {
			return [];
		}
		return `${words(name)} `.startsWith(`${words(text)} `)
			? []
			: [`name-starts-with-text: "${text}" is named "${name}"`];
	});
};
