import type { JSX } from "solid-js";
import { splitProps } from "solid-js";

export type TextAreaProps = JSX.TextareaHTMLAttributes<HTMLTextAreaElement>;

/** A multi-line text input. Controlled: pass `value` and `onInput`. */
const TextArea = (props: TextAreaProps) => {
	const [local, rest] = splitProps(props, ["class"]);
	return (
		<textarea
			class={
				local.class === undefined ? "ui-textarea" : `ui-textarea ${local.class}`
			}
			{...rest}
		/>
	);
};

export default TextArea;
