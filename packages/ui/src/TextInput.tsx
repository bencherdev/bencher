import type { JSX } from "solid-js";
import { splitProps } from "solid-js";

export interface TextInputProps
	extends Omit<JSX.InputHTMLAttributes<HTMLInputElement>, "size"> {
	size?: "sm" | "md" | "lg";
}

/** A single-line text input. Controlled: pass `value` and `onInput`. */
const TextInput = (props: TextInputProps) => {
	const [local, rest] = splitProps(props, ["size", "class", "type"]);
	return (
		<input
			type={local.type ?? "text"}
			class={local.class === undefined ? "ui-input" : `ui-input ${local.class}`}
			data-size={local.size ?? "md"}
			{...rest}
		/>
	);
};

export default TextInput;
