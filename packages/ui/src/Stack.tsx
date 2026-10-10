import type { JSX } from "solid-js";
import { splitProps } from "solid-js";

export interface StackProps extends JSX.HTMLAttributes<HTMLDivElement> {
	direction?: "column" | "row";
	/** A spacing step: gap 3 is var(--spacing-3). */
	gap?: 0 | 1 | 2 | 3 | 4 | 5 | 6 | 8 | 10 | 12;
	align?: "start" | "center" | "end";
	justify?: "between" | "center" | "end";
	wrap?: boolean;
}

/** Flex layout from spacing tokens: the only spacing app code needs. */
const Stack = (props: StackProps) => {
	const [local, rest] = splitProps(props, [
		"direction",
		"gap",
		"align",
		"justify",
		"wrap",
		"class",
	]);
	return (
		<div
			class={local.class === undefined ? "ui-stack" : `ui-stack ${local.class}`}
			data-direction={local.direction ?? "column"}
			data-gap={String(local.gap ?? 2)}
			data-align={local.align}
			data-justify={local.justify}
			data-wrap={local.wrap === true ? "" : undefined}
			{...rest}
		/>
	);
};

export default Stack;
