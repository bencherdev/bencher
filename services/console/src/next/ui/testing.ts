import { expect } from "vitest";

/**
 * The text a node shows, read as voice control and axe's label-in-name check
 * read it: the text nodes of what is drawn, in order, each element's own text trimmed, so a space
 * inside a child's edge is lost and two parts with none between them run together.
 */
export const shownText = (node: Node): string =>
	node.nodeType === Node.TEXT_NODE
		? (node.nodeValue ?? "")
		: node instanceof Element && !node.checkVisibility()
			? ""
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

/**
 * Each header cell's left edge and width, and those of the first row drawn
 * under them, which a table whose columns never follow its rows keeps as it scrolls.
 */
export const columnEdges = (table: Element) => {
	const edges = (cells: Iterable<Element>) =>
		[...cells].map((cell) => {
			const box = cell.getBoundingClientRect();
			return `${Math.round(box.left)}+${Math.round(box.width)}`;
		});
	return {
		headers: edges(table.querySelectorAll("thead th")),
		cells: edges(
			table.querySelector("tbody tr[aria-rowindex]")?.children ?? [],
		),
	};
};

/**
 * How `table` holds its names: `held` with its name column at least 16
 * characters (`--size-name-min`) wide, `folded` with every cut name in a
 * folded row that wide, or the width a name falls to; and whether its own
 * track scrolls sideways.
 */
export const nameFloor = (table: Element, column: string) => {
	const scroll = table.closest(".ui-table-scroll") as HTMLElement;
	const sideways = scroll.scrollWidth > scroll.clientWidth;
	const names = [
		...table.querySelectorAll<HTMLElement>('tbody td[data-fold="l1"]'),
	];
	if (names.length === 0) {
		return { name: "no rows", sideways };
	}
	const probe = document.createElement("span");
	probe.style.cssText =
		"display: block; width: var(--size-name-min); font-weight: normal";
	names[0]?.append(probe);
	const floor = probe.getBoundingClientRect().width - 0.5;
	probe.remove();
	if (getComputedStyle(table).display === "block") {
		const cut = names.filter((cell) => {
			const text = document.createRange();
			text.selectNodeContents(cell);
			return text.getBoundingClientRect().width > cell.clientWidth + 0.5;
		});
		const width = Math.min(...cut.map((cell) => cell.clientWidth));
		return {
			name: width >= floor ? "folded" : `folded ${Math.round(width)}px`,
			sideways,
		};
	}
	const header = [...table.querySelectorAll("thead th")].find(
		(th) => th.textContent?.trim() === column,
	) as HTMLElement;
	const style = getComputedStyle(header);
	const width =
		header.getBoundingClientRect().width -
		Number.parseFloat(style.paddingLeft) -
		Number.parseFloat(style.paddingRight);
	return {
		name: width >= floor ? "held" : `${Math.round(width)}px`,
		sideways,
	};
};

/** Sizes the page so `table`'s own box is `width` wide, the screen unchanged. */
export const sizeTo = (table: Element, width: number) => {
	const scroll = table.closest(".ui-table-scroll") as HTMLElement;
	document.body.style.width = "";
	document.body.style.width = `${document.body.clientWidth - scroll.clientWidth + width}px`;
	expect(scroll.clientWidth).toBe(width);
};

/** The lines each of `table`'s rows takes, as `onLines` reads them. */
export const linesOf = (table: Element) =>
	Number(getComputedStyle(table).getPropertyValue("--ui-table-lines")) || 1;

/** The widths every table family holds its name column at, or folds. */
export const FLOOR_WIDTHS = [320, 768, 1024, 1440] as const;

/** Names near their caps: a branch of 252 bytes in many segments, and 64 bytes with no break. */
export const STRESS = {
	branch: `release/${"subsystem/".repeat(23)}very-long-leaf`,
	name: "a".repeat(64),
} as const;

/** Asserts `table` folds with its box a pixel under `fold` and holds its `column` at least 16 characters wide a pixel over. */
export const expectFoldAt = (table: Element, column: string, fold: number) => {
	sizeTo(table, fold - 1);
	expect(getComputedStyle(table).display, `at ${fold - 1}`).toBe("block");
	sizeTo(table, fold + 1);
	expect(nameFloor(table, column), `at ${fold + 1}`).toEqual({
		name: "held",
		sideways: false,
	});
	document.body.style.width = "";
};

/** Asserts `table`'s folded rows take `lines` lines with its box `width` wide and one fewer a pixel wider, each holding 16 characters of name. */
export const expectLinesAt = (table: Element, width: number, lines: number) => {
	for (const [at, taken] of [
		[width, lines],
		[width + 1, lines - 1],
	] as const) {
		sizeTo(table, at);
		expect(linesOf(table), `at ${at}`).toBe(taken);
		expect(nameFloor(table, ""), `at ${at}`).toEqual({
			name: "folded",
			sideways: false,
		});
	}
	document.body.style.width = "";
};
