import type { JSX } from "solid-js";
import { Show, splitProps } from "solid-js";
import { TruncateChild } from "./Truncate";

export type TokenTone =
	| "blue"
	| "teal"
	| "green"
	| "yellow"
	| "orange"
	| "red"
	| "pink"
	| "purple"
	| "gray";

export interface TokenProps extends JSX.HTMLAttributes<HTMLSpanElement> {
	/** A hue means something enumerable; the default is neutral. */
	tone?: TokenTone;
	/** `strong` inverts for a high-contrast chip. */
	variant?: "soft" | "strong";
	/** Lead with a status dot. */
	dot?: boolean;
}

/** A small labeled chip for enumerated metadata, never decoration. */
const Token = (props: TokenProps) => {
	const [local, rest] = splitProps(props, [
		"tone",
		"variant",
		"dot",
		"class",
		"children",
	]);
	return (
		<span
			class={local.class === undefined ? "ui-token" : `ui-token ${local.class}`}
			data-tone={local.tone}
			data-variant={local.variant}
			{...rest}
		>
			<Show when={local.dot === true}>
				<span class="ui-token-dot" />
			</Show>
			<TruncateChild reveal>{local.children}</TruncateChild>
		</span>
	);
};

export default Token;
