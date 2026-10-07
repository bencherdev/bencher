import type { JSX } from "solid-js";
import { splitProps } from "solid-js";

export type ListProps = JSX.HTMLAttributes<HTMLDivElement>;

/** Dense data renders as rows: an edge-to-edge group with dividers. */
const List = (props: ListProps) => {
	const [local, rest] = splitProps(props, ["class"]);
	return (
		<div
			class={local.class === undefined ? "ui-list" : `ui-list ${local.class}`}
			{...rest}
		/>
	);
};

export default List;
