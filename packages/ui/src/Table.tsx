import type { JSX } from "solid-js";
import { splitProps } from "solid-js";

export type TableProps = JSX.HTMLAttributes<HTMLTableElement>;

/**
 * Dense tabular data: columns under a header row, with dividers. The table
 * scrolls inside its own track on a narrow screen, so the page never does.
 * Compose it with the semantic table elements: `thead`, `tbody`, `tr`, `th`,
 * `td`.
 */
const Table = (props: TableProps) => {
	const [local, rest] = splitProps(props, ["class"]);
	return (
		<div class="ui-table-scroll">
			<table
				class={
					local.class === undefined ? "ui-table" : `ui-table ${local.class}`
				}
				{...rest}
			/>
		</div>
	);
};

export default Table;
