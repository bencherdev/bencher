import type { JSX } from "solid-js";
import { onCleanup, onMount, splitProps } from "solid-js";
import { watchSize } from "./resize";

export interface TableProps extends JSX.HTMLAttributes<HTMLTableElement> {
	/** The container width, in px, at or below which each row folds into two lines, placed by its cells' `data-fold`. */
	fold?: number;
	/** Told the lines each row takes, on first layout and whenever that changes, a frame late: 1, 2 once folded, or the `--ui-table-lines` a table that restacks its rows sets. */
	onLines?: (lines: number) => void;
}

/**
 * Dense tabular data: columns under a header row, with dividers. The table
 * scrolls inside its own track when its container is narrow, so the page
 * never does. Compose it with the semantic table elements: `thead`, `tbody`,
 * `tr`, `th`, `td`. A cell of user text whose width follows its content takes
 * `ui-table-name`, so it gives way to the others down to the name floor.
 */
const Table = (props: TableProps) => {
	const [local, rest] = splitProps(props, ["class", "fold", "onLines"]);
	let scroll: HTMLDivElement | undefined;
	let table: HTMLTableElement | undefined;
	onMount(() => {
		const onLines = local.onLines;
		if (scroll === undefined || onLines === undefined) {
			return;
		}
		let lines: number | undefined;
		onCleanup(
			watchSize(scroll, () => {
				const now =
					table === undefined
						? 1
						: Number(
								getComputedStyle(table).getPropertyValue("--ui-table-lines"),
							) || 1;
				if (now === lines) {
					return undefined;
				}
				lines = now;
				// The rows change height with their lines, which resizes the scroll.
				return () => requestAnimationFrame(() => onLines(now));
			}),
		);
	});
	return (
		<div
			ref={scroll}
			class="ui-table-scroll"
			style={
				local.fold === undefined
					? undefined
					: { "--ui-table-fold": `${local.fold}px` }
			}
		>
			<table
				ref={table}
				class={
					local.class === undefined ? "ui-table" : `ui-table ${local.class}`
				}
				data-fold={local.fold === undefined ? undefined : ""}
				{...rest}
			/>
		</div>
	);
};

export default Table;
