import Button from "@bencherdev/ui/Button";
import { Show } from "solid-js";

/** A link to the neighbor on one side, or that side's control, disabled, while there is none. */
const Neighbor = (props: {
	side: "previous" | "next";
	link: NeighborLink | undefined;
	/** Moving onto, focusing, or touching the link, before the click. */
	onIntent?: () => void;
}) => {
	const text = () => (props.side === "previous" ? "Previous" : "Next");
	const chevron = () => (
		<svg class="rp-chevron" viewBox="0 0 24 24" aria-hidden="true">
			<path d={props.side === "previous" ? "M15 5l-7 7 7 7" : "M9 5l7 7-7 7"} />
		</svg>
	);
	const intent = () => props.onIntent?.();
	return (
		<Show
			when={props.link}
			fallback={
				<Button size="sm" class="rp-neighbor" disabled>
					{text()}
				</Button>
			}
		>
			{(link) => (
				<a
					class="ui-button rp-neighbor"
					data-variant="secondary"
					data-size="sm"
					href={link().href}
					aria-label={`${text()} ${link().shown}: ${link().detail}`}
					onPointerMove={onMoved(intent)}
					onFocus={intent}
					onTouchStart={intent}
				>
					<Show when={props.side === "previous"}>{chevron()}</Show>
					{text()} <span class="muted">{link().shown}</span>
					<Show when={props.side === "next"}>{chevron()}</Show>
				</a>
			)}
		</Show>
	);
};

export default Neighbor;

/** Calls `intent` when the pointer itself moves over an element, never when the page changes under a still pointer. */
export const onMoved = (intent: () => void) => (event: PointerEvent) => {
	if (event.movementX || event.movementY) {
		intent();
	}
};

/** Where a Previous or Next control leads, what it shows after its word, and the rest of its name. */
interface NeighborLink {
	href: string;
	shown: string;
	detail: string;
}
