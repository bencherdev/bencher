import type { JSX } from "solid-js";
import { Show, splitProps } from "solid-js";

export interface EmptyStateProps extends JSX.HTMLAttributes<HTMLDivElement> {
	title: string;
	/** One quiet sentence of guidance, in children. */
}

/** The empty region, saying so plainly. */
const EmptyState = (props: EmptyStateProps) => {
	const [local, rest] = splitProps(props, ["title", "class", "children"]);
	return (
		<div
			class={
				local.class === undefined
					? "ui-empty-state"
					: `ui-empty-state ${local.class}`
			}
			{...rest}
		>
			<p class="ui-empty-state-title">{local.title}</p>
			<Show when={local.children}>
				<div class="ui-empty-state-description">{local.children}</div>
			</Show>
		</div>
	);
};

export default EmptyState;
