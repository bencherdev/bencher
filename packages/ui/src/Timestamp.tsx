import type { JSX } from "solid-js";
import { Show, splitProps } from "solid-js";

export interface TimestampProps extends JSX.HTMLAttributes<HTMLElement> {
	/** The full, precise time; shown on hover. */
	title?: string;
	/** With an href the timestamp is the permalink. */
	href?: string;
	datetime?: string;
}

/** A quiet relative time. Children carry the formatted text. */
const Timestamp = (props: TimestampProps) => {
	const [local, rest] = splitProps(props, [
		"title",
		"href",
		"datetime",
		"class",
	]);
	const classes = () =>
		local.class === undefined ? "ui-timestamp" : `ui-timestamp ${local.class}`;
	return (
		<Show
			when={local.href}
			fallback={
				<time
					class={classes()}
					title={local.title}
					datetime={local.datetime}
					{...(rest as JSX.TimeHTMLAttributes<HTMLTimeElement>)}
				/>
			}
		>
			{(href) => (
				<a
					class={classes()}
					href={href()}
					title={local.title}
					{...(rest as JSX.AnchorHTMLAttributes<HTMLAnchorElement>)}
				/>
			)}
		</Show>
	);
};

export default Timestamp;
