import Truncate from "@bencherdev/ui/Truncate";
import { Show } from "solid-js";

/** A `key=value` parameter as a pill whose key and value share its width, so the value is never the part cut first. */
const Tag = (props: { tag: string; reveal?: boolean; class?: string }) => {
	const at = () => props.tag.indexOf("=");
	return (
		<span class={`lr-ptag ui-parts ${props.class ?? ""}`}>
			<Show
				when={at() >= 0}
				fallback={<Truncate text={props.tag} reveal={props.reveal === true} />}
			>
				<Truncate
					text={props.tag.slice(0, at())}
					reveal={props.reveal === true}
				/>
				<span data-fixed>=</span>
				<Truncate
					text={props.tag.slice(at() + 1)}
					reveal={props.reveal === true}
				/>
			</Show>
		</span>
	);
};

export default Tag;
