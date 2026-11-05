import {
	type Accessor,
	createComputed,
	createEffect,
	createMemo,
	createSignal,
	on,
	onCleanup,
	onMount,
} from "solid-js";
import type { Range } from "./rows";
import { holdRows, markRows, useVirtualRows } from "./virtual";

/** Where `drawn` holds the spacer between the opened row and the rows near the screen. */
export const GAP: unique symbol = Symbol("gap");

/**
 * `useVirtualRows` for a table whose names open in place: one name open at a
 * time, its row kept drawn on whichever side of the rows near the screen it
 * scrolls to, behind a spacer, so it keeps its height and its open state and
 * nothing on screen moves.
 */
export const useKeptRows = <T>(props: {
	rows: Accessor<readonly T[]>;
	onRange?: ((range: Range) => void) | undefined;
}) => {
	const rows = useVirtualRows(props);
	let body: HTMLTableSectionElement | undefined;
	const [opened, setOpened] = createSignal<T>();
	/** Where the opened row is, while it is outside the rows near the screen. */
	const kept = createMemo(() => {
		const row = opened();
		const at = row === undefined ? -1 : props.rows().indexOf(row);
		const { start, end } = rows.range();
		return at < 0 || (at >= start && at < end) ? undefined : at;
	});
	const before = () => {
		const at = kept();
		return at !== undefined && at < rows.range().start;
	};
	const drawn = createMemo((): (T | typeof GAP)[] => {
		const at = kept();
		const row = at === undefined ? undefined : props.rows()[at];
		if (row === undefined) {
			return rows.shown();
		}
		return before() ? [row, GAP, ...rows.shown()] : [...rows.shown(), GAP, row];
	});
	let closing = false;
	const onClick = (event: MouseEvent) => {
		const target = event.target as Element;
		const row = target.closest("tr[aria-rowindex]");
		if (closing || !(body && row && target.closest("button.ui-trunc"))) {
			return;
		}
		// After the click has opened or closed its name.
		requestAnimationFrame(() => {
			const lines = [...(body?.querySelectorAll("tr[aria-rowindex]") ?? [])];
			// A name closed above the screen lifts the rows on it, so the window
			// follows the first of them in the same frame.
			const mark = markRows(body);
			const open = row.querySelector("[data-open]");
			if (open) {
				closing = true;
				for (const other of body?.querySelectorAll<HTMLElement>(
					"[data-open]",
				) ?? []) {
					if (other !== open) {
						other.click();
					}
				}
				closing = false;
				const at = lines.indexOf(row);
				setOpened(() => drawn().filter((each) => each !== GAP)[at] as T);
			} else {
				setOpened(undefined);
			}
			holdRows(body, mark);
		});
	};
	// The opened row moves to its side of the gap, and a node that moves loses
	// focus, so the focus it held comes back.
	let held: Element | null = null;
	createComputed(
		on(drawn, () => {
			held = document.activeElement;
		}),
	);
	createEffect(
		on(drawn, () => {
			if (
				held instanceof HTMLElement &&
				held !== document.activeElement &&
				body?.contains(held)
			) {
				held.focus({ preventScroll: true });
			}
		}),
	);
	onMount(() => {
		body?.addEventListener("click", onClick);
		onCleanup(() => body?.removeEventListener("click", onClick));
	});
	return {
		...rows,
		body: (element: HTMLTableSectionElement) => {
			body = element;
			// WebKit anchors in whole pixels after the window follows, moving rows by up to 2px.
			element.style.overflowAnchor = "none";
			rows.body(element);
		},
		/** The rows to draw: those near the screen, and the opened row and a `GAP` on its side. */
		drawn,
		/** The index in the list of the row drawn at `position`. */
		index: (position: number) => {
			const at = kept();
			const { start } = rows.range();
			if (at === undefined) {
				return start + position;
			}
			if (before()) {
				return position === 0 ? at : start + position - 2;
			}
			return position === drawn().length - 1 ? at : start + position;
		},
		above: () => rows.spaceFor(before() ? (kept() ?? 0) : rows.range().start),
		/** The rows between the opened row and those near the screen. */
		gap: () => {
			const at = kept();
			if (at === undefined) {
				return 0;
			}
			const { start, end } = rows.range();
			return (before() ? start - at - 1 : at - end) * rows.rowHeight();
		},
		below: () => {
			const at = kept();
			const last = at === undefined || before() ? rows.range().end : at + 1;
			return (props.rows().length - last) * rows.rowHeight();
		},
	};
};
