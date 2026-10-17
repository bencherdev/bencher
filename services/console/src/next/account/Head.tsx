import Heading from "@bencherdev/ui/Heading";
import { For, type JSX, Show } from "solid-js";

/** An account page's crumb, title, and actions. */
const Head = (props: {
	title: string;
	/** Crumbs after Account. */
	crumbs?: { label: string; href: string }[];
	actions?: JSX.Element;
	/** The title, for focus to fall back on. */
	ref?: (heading: HTMLHeadingElement) => void;
}) => (
	<div class="sethead">
		<div class="ph-title">
			<div class="crumb-line">
				Account<span aria-hidden="true">/</span>
				<For each={props.crumbs}>
					{(crumb) => (
						<>
							<a href={crumb.href}>{crumb.label}</a>
							<span aria-hidden="true">/</span>
						</>
					)}
				</For>
			</div>
			<Heading
				level={1}
				size="xl"
				ref={props.ref}
				tabindex={props.ref ? -1 : undefined}
			>
				{props.title}
			</Heading>
		</div>
		<Show when={props.actions}>
			<div class="ph-actions">{props.actions}</div>
		</Show>
	</div>
);

export default Head;
