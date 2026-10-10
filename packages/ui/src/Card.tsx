import type { JSX } from "solid-js";
import { splitProps } from "solid-js";
import { Dynamic } from "solid-js/web";

export interface CardProps extends JSX.HTMLAttributes<HTMLElement> {
	/** The rendered element. A card is a self-contained unit, so it
	 * defaults to `article`; a form card is `as="form"`. */
	as?: "article" | "div" | "section" | "form";
}

/** A surface for one self-contained unit. Never a list-row wrapper. */
const Card = (props: CardProps) => {
	const [local, rest] = splitProps(props, ["as", "class"]);
	return (
		<Dynamic
			component={local.as ?? "article"}
			class={local.class === undefined ? "ui-card" : `ui-card ${local.class}`}
			{...rest}
		/>
	);
};

export default Card;
