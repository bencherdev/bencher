import type { JSX } from "solid-js";
import { splitProps } from "solid-js";

export interface TextInputProps
	extends Omit<JSX.InputHTMLAttributes<HTMLInputElement>, "size"> {
	size?: "sm" | "md" | "lg";
	/** A slug: a touch keyboard opens lowercase, with no corrections. */
	slug?: boolean;
}

/** A single-line text input. Controlled: pass `value` and `onInput`. */
const TextInput = (props: TextInputProps) => {
	const [local, rest] = splitProps(props, ["size", "class", "type", "slug"]);
	return (
		<input
			type={local.type ?? "text"}
			class={local.class === undefined ? "ui-input" : `ui-input ${local.class}`}
			data-size={local.size ?? "md"}
			autocapitalize={local.slug ? "none" : undefined}
			autocorrect={local.slug ? "off" : undefined}
			spellcheck={local.slug ? false : undefined}
			autocomplete={local.slug ? "off" : undefined}
			{...rest}
		/>
	);
};

export default TextInput;
