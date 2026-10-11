import type { JSX } from "solid-js";
import { Show, splitProps } from "solid-js";

export interface ListItemProps
	extends JSX.AnchorHTMLAttributes<HTMLAnchorElement> {
	/** With an href the whole row is the link. */
	href?: string;
}

/** One row in a List: a single scannable line, thumb-sized. */
const ListItem = (props: ListItemProps) => {
	const [local, rest] = splitProps(props, ["href", "class"]);
	const classes = () =>
		local.class === undefined ? "ui-list-item" : `ui-list-item ${local.class}`;
	return (
		<Show
			when={local.href}
			fallback={
				<div
					class={classes()}
					{...(rest as JSX.HTMLAttributes<HTMLDivElement>)}
				/>
			}
		>
			{(href) => <a href={href()} class={classes()} {...rest} />}
		</Show>
	);
};

export default ListItem;
