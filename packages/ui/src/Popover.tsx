import type { JSX } from "solid-js";
import { splitProps } from "solid-js";

export interface PopoverProps extends JSX.HTMLAttributes<HTMLDetailsElement> {
	/** The trigger content, rendered as a button-styled summary. */
	summary: JSX.Element;
	/** Screen reader label for the trigger. */
	summaryLabel?: string;
	summaryVariant?: "primary" | "secondary" | "ghost" | "destructive" | "muted";
	summarySize?: "sm" | "md" | "lg";
	/** Menu shape; `pill` suits a single row of chips. */
	shape?: "menu" | "pill";
	ref?: (element: HTMLDetailsElement) => void;
}

/**
 * A small anchored menu on a details/summary disclosure: no scripts, no
 * portal, dismissed by its own toggle. Close it programmatically through
 * `ref` by setting `open = false`.
 */
const Popover = (props: PopoverProps) => {
	const [local, rest] = splitProps(props, [
		"summary",
		"summaryLabel",
		"summaryVariant",
		"summarySize",
		"shape",
		"class",
		"children",
	]);
	return (
		<details
			class={
				local.class === undefined ? "ui-popover" : `ui-popover ${local.class}`
			}
			{...rest}
		>
			<summary
				class="ui-button"
				data-variant={local.summaryVariant ?? "secondary"}
				data-size={local.summarySize ?? "sm"}
				aria-label={local.summaryLabel}
			>
				{local.summary}
			</summary>
			<div class="ui-popover-menu" data-shape={local.shape ?? "menu"}>
				{local.children}
			</div>
		</details>
	);
};

export default Popover;
