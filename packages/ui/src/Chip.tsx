import type { JSX } from "solid-js";
import { Show, splitProps } from "solid-js";
import Icon from "./Icon";
import { TruncateChild } from "./Truncate";

export interface ChipProps extends JSX.ButtonHTMLAttributes<HTMLButtonElement> {
	/** A quiet key before the value, such as `branch` in `branch main`. */
	label?: string;
	/** Set when the chip narrows something, so it stands out from the defaults. */
	on?: boolean;
	/** A caret, for a chip that opens a menu or a sheet. */
	caret?: boolean;
}

/** A rounded button that shows a setting and opens what changes it. */
const Chip = (props: ChipProps) => {
	const [local, rest] = splitProps(props, [
		"label",
		"on",
		"caret",
		"class",
		"children",
	]);
	return (
		<button
			type="button"
			class={local.class === undefined ? "ui-chip" : `ui-chip ${local.class}`}
			data-on={local.on ? "" : undefined}
			{...rest}
		>
			<Show
				when={local.label}
				fallback={<TruncateChild>{local.children}</TruncateChild>}
			>
				<span class="ui-parts">
					<span class="ui-chip-key" data-fixed>
						{local.label}
					</span>{" "}
					<TruncateChild>{local.children}</TruncateChild>
				</span>
			</Show>
			<Show when={local.caret}>
				<Icon name="chevron-down" class="ui-chip-caret" />
			</Show>
		</button>
	);
};

export default Chip;
