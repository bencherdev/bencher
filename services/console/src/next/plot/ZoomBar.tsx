import Button from "@bencherdev/ui/Button";
import Tooltip from "@bencherdev/ui/Tooltip";
import type { JSX } from "solid-js";

// Styled here rather than in the console's one stylesheet, which every page loads, as only a plot's pages draw it.
const BAR: JSX.CSSProperties = {
	display: "inline-flex",
	"align-items": "center",
	gap: "2px",
	"min-width": "0",
	margin: "0",
	padding: "0",
	border: "0",
};
const SPAN: JSX.CSSProperties = {
	"margin-inline-end": "var(--spacing-1)",
	color: "var(--color-text-secondary)",
	"font-size": "var(--font-size-xs)",
	"white-space": "nowrap",
};

/** The span a plot is zoomed to, then Zoom out and Reset zoom. */
const ZoomBar = (props: {
	/** The span in view, as "Oct 3 to Oct 9". */
	span: string;
	/** The span's class where the page names its window, else muted text. */
	class?: string | undefined;
	out: () => void;
	reset: () => void;
	/** Puts the focus on the plot area. */
	focus: () => void;
}) => {
	// A step back that ends the zoom takes these buttons away, so the focus goes to the plot area.
	const act = (button: HTMLButtonElement, step: () => void) => {
		step();
		if (!button.isConnected) {
			props.focus();
		}
	};
	return (
		<fieldset style={BAR} aria-label="Zoom">
			<span class={props.class} style={props.class ? undefined : SPAN}>
				{props.span}
			</span>
			<ZoomButton label="Zoom out" onClick={(button) => act(button, props.out)}>
				<circle cx="10.5" cy="10.5" r="6.5" />
				<path d="M20 20l-4.9-4.9M7.5 10.5h6" />
			</ZoomButton>
			<ZoomButton
				label="Reset zoom"
				onClick={(button) => act(button, props.reset)}
			>
				<path d="M4 9V4h5M15 4h5v5M20 15v5h-5M9 20H4v-5" />
			</ZoomButton>
		</fieldset>
	);
};

const ZoomButton = (props: {
	label: string;
	onClick: (button: HTMLButtonElement) => void;
	children: JSX.Element;
}) => (
	<Tooltip tip={props.label}>
		<Button
			variant="ghost"
			size="sm"
			iconOnly
			aria-label={props.label}
			onClick={(event) => props.onClick(event.currentTarget)}
		>
			<svg
				viewBox="0 0 24 24"
				width="16"
				height="16"
				fill="none"
				stroke="currentColor"
				stroke-width="2"
				stroke-linecap="round"
				stroke-linejoin="round"
				aria-hidden="true"
			>
				{props.children}
			</svg>
		</Button>
	</Tooltip>
);

export default ZoomBar;
