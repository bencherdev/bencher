import Heading from "@bencherdev/ui/Heading";
import Icon from "@bencherdev/ui/Icon";
import { type JSX, Show } from "solid-js";
import { NEXT_PROJECTS } from "../paths";

/** A section's crumb, title, and the read-only line for a reader who cannot change it. */
const Head = (props: {
	slug: string;
	title: string;
	readOnly: boolean;
	children?: JSX.Element;
	actions?: JSX.Element;
}) => (
	<div class="sethead">
		<div class="ph-title">
			<div class="crumb-line">
				<a href={`${NEXT_PROJECTS}/${props.slug}/settings`}>Settings</a>
				<span aria-hidden="true">/</span>
			</div>
			<Heading level={1} size="xl">
				{props.title}
			</Heading>
			{props.children}
			<Show when={props.readOnly}>
				<p class="ro-note">
					<Icon name="read-only" />
					Read only. Ask a project Maintainer for access.
				</p>
			</Show>
		</div>
		<Show when={props.actions}>
			<div class="ph-actions">{props.actions}</div>
		</Show>
	</div>
);

export default Head;
