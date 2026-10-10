import type { JSX } from "solid-js";
import { splitProps } from "solid-js";
import { Dynamic } from "solid-js/web";

export interface HeadingProps extends JSX.HTMLAttributes<HTMLHeadingElement> {
	/** The semantic level: h1 through h6. */
	level?: 1 | 2 | 3 | 4 | 5 | 6;
	/** The visual size, independent of the level. */
	size?: "sm" | "base" | "lg" | "xl";
}

/** A heading whose look and semantics are set separately. */
const Heading = (props: HeadingProps) => {
	const [local, rest] = splitProps(props, ["level", "size", "class"]);
	return (
		<Dynamic
			component={`h${local.level ?? 2}`}
			class={
				local.class === undefined ? "ui-heading" : `ui-heading ${local.class}`
			}
			data-size={local.size ?? "base"}
			{...rest}
		/>
	);
};

export default Heading;
