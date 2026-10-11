import type { JSX } from "solid-js";
import { Show, splitProps } from "solid-js";

export interface FieldProps extends JSX.HTMLAttributes<HTMLDivElement> {
	/** Visible label text. Inputs without one carry `aria-label`. */
	label?: string;
	/** The labeled control's id. */
	for?: string;
}

/** One labeled control: a label over its input. */
const Field = (props: FieldProps) => {
	const [local, rest] = splitProps(props, [
		"label",
		"for",
		"class",
		"children",
	]);
	return (
		<div
			class={local.class === undefined ? "ui-field" : `ui-field ${local.class}`}
			{...rest}
		>
			<Show when={local.label}>
				{(label) => (
					<label class="ui-label" for={local.for}>
						{label()}
					</label>
				)}
			</Show>
			{local.children}
		</div>
	);
};

export default Field;
