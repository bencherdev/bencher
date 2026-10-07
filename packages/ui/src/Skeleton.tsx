import type { JSX } from "solid-js";
import { splitProps } from "solid-js";

export interface SkeletonProps extends JSX.HTMLAttributes<HTMLDivElement> {
	/** `text` is one line of the surrounding type; its width is the caller's. */
	size?: "text" | "row" | "card";
}

/** A quiet placeholder while real content loads. */
const Skeleton = (props: SkeletonProps) => {
	const [local, rest] = splitProps(props, ["size", "class"]);
	return (
		<div
			class={
				local.class === undefined ? "ui-skeleton" : `ui-skeleton ${local.class}`
			}
			data-size={local.size ?? "row"}
			aria-hidden="true"
			{...rest}
		/>
	);
};

export default Skeleton;
