import type { JSX } from "solid-js";
import { createRenderEffect, splitProps } from "solid-js";

export interface SelectorProps
	extends Omit<JSX.SelectHTMLAttributes<HTMLSelectElement>, "size"> {
	size?: "sm" | "md" | "lg";
}

/**
 * A native select. Controlled: pass `value` and `onChange`.
 *
 * `value` is applied through a render effect after the `<option>` children
 * mount, never as an initial attribute. A native select resolves its value
 * against the options present at the moment the property is set: assign it
 * while the option list is still empty and the browser silently falls back
 * to the first option, so a controlled value handed in before its options
 * would be ignored. The effect reasserts the value whenever it or the option
 * set changes, keeping the display and the controlled value in agreement.
 */
const Selector = (props: SelectorProps) => {
	const [local, rest] = splitProps(props, [
		"size",
		"class",
		"children",
		"value",
	]);
	let select: HTMLSelectElement | undefined;
	createRenderEffect(() => {
		// Re-run when the option set changes so the value is reasserted
		// against the freshly mounted options.
		void local.children;
		const value = local.value;
		if (select !== undefined && value !== undefined) {
			select.value = String(value);
		}
	});
	return (
		<span
			class={
				local.class === undefined
					? "ui-select-wrap"
					: `ui-select-wrap ${local.class}`
			}
		>
			<select
				ref={(element) => {
					select = element;
				}}
				class="ui-select"
				data-size={local.size ?? "md"}
				{...rest}
			>
				{local.children}
			</select>
		</span>
	);
};

export default Selector;
