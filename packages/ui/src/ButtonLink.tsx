import type { JSX } from "solid-js";
import { splitProps } from "solid-js";
import type { ButtonProps } from "./Button";

export interface ButtonLinkProps
	extends JSX.AnchorHTMLAttributes<HTMLAnchorElement> {
	variant?: ButtonProps["variant"];
	size?: ButtonProps["size"];
	full?: boolean;
}

/** Navigation drawn as a Button: a link, never an action. */
const ButtonLink = (props: ButtonLinkProps) => {
	const [local, rest] = splitProps(props, ["variant", "size", "full", "class"]);
	return (
		<a
			class={
				local.class === undefined ? "ui-button" : `ui-button ${local.class}`
			}
			data-variant={local.variant ?? "secondary"}
			data-size={local.size ?? "md"}
			data-full={local.full === true ? "" : undefined}
			{...rest}
		/>
	);
};

export default ButtonLink;
