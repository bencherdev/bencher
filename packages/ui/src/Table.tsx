import type { JSX } from "solid-js";
import { splitProps } from "solid-js";

export interface TableProps extends JSX.HTMLAttributes<HTMLTableElement> {
	/** Fold each row into two lines on a narrow screen, placed by its cells' `data-fold`. */
	fold?: boolean;
}

/**
 * Dense tabular data: columns under a header row, with dividers. The table
 * scrolls inside its own track on a narrow screen, so the page never does.
 * Compose it with the semantic table elements: `thead`, `tbody`, `tr`, `th`,
 * `td`.
 */
const Table = (props: TableProps) => {
	const [local, rest] = splitProps(props, ["class", "fold"]);
	return (
		<div class="ui-table-scroll">
			<table
				class={
					local.class === undefined ? "ui-table" : `ui-table ${local.class}`
				}
				data-fold={local.fold === true ? "" : undefined}
				{...rest}
			/>
		</div>
	);
};

export default Table;
