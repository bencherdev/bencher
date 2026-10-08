import type { JSX } from "solid-js";
import { splitProps } from "solid-js";

export interface ButtonProps
	extends JSX.ButtonHTMLAttributes<HTMLButtonElement> {
	/** The one filled accent button per view is `primary`. */
	variant?:
		| "primary"
		| "secondary"
		| "ghost"
		| "danger"
		| "destructive"
		| "muted";
	size?: "sm" | "md" | "lg";
	/** Stretch to the container width. */
	full?: boolean;
	/** Toggle state, for chips. Renders `aria-pressed`. */
	selected?: boolean;
}

/** An action. Buttons act; navigation is a link. */
const Button = (props: ButtonProps) => {
	const [local, rest] = splitProps(props, [
		"variant",
		"size",
		"full",
		"selected",
		"class",
		"type",
	]);
	return (
		<button
			type={local.type ?? "button"}
			class={
				local.class === undefined ? "ui-button" : `ui-button ${local.class}`
			}
			data-variant={local.variant ?? "secondary"}
			data-size={local.size ?? "md"}
			data-full={local.full === true ? "" : undefined}
			data-selected={local.selected === true ? "" : undefined}
			aria-pressed={local.selected}
			{...rest}
		/>
	);
};

export default Button;
