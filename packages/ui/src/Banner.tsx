import type { JSX } from "solid-js";
import { splitProps } from "solid-js";

export interface BannerProps extends JSX.HTMLAttributes<HTMLDivElement> {
	status?: "neutral" | "info" | "success" | "warning" | "error";
}

/** Inline feedback that lives in the surface it describes. Never a toast. */
const Banner = (props: BannerProps) => {
	const [local, rest] = splitProps(props, ["status", "class"]);
	return (
		<div
			class={
				local.class === undefined ? "ui-banner" : `ui-banner ${local.class}`
			}
			data-status={local.status ?? "neutral"}
			{...rest}
		/>
	);
};

export default Banner;
