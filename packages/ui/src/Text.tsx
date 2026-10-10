import type { JSX } from "solid-js";
import { splitProps } from "solid-js";
import { Dynamic } from "solid-js/web";

export interface TextProps extends JSX.HTMLAttributes<HTMLElement> {
	as?: "span" | "p" | "a";
	size?: "xs" | "sm" | "base";
	tone?: "primary" | "secondary" | "disabled";
	/** Renders only when `as="a"`. */
	href?: string;
}

/** A run of text with its size and tone named, never hardcoded. */
const Text = (props: TextProps) => {
	const [local, rest] = splitProps(props, [
		"as",
		"size",
		"tone",
		"href",
		"class",
	]);
	return (
		<Dynamic
			component={local.as ?? "span"}
			class={local.class === undefined ? "ui-text" : `ui-text ${local.class}`}
			data-size={local.size ?? "base"}
			data-tone={local.tone ?? "primary"}
			href={local.as === "a" ? local.href : undefined}
			{...rest}
		/>
	);
};

export default Text;
