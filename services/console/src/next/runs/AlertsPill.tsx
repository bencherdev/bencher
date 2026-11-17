import { type JSX, Show } from "solid-js";

/** A run's or a report's alerts as a pill: red with a dot once it raised one, else muted, so the pills' line is never empty. */
const AlertsPill = (props: { total: number; children: JSX.Element }) => (
	<span class="rp-alerts" data-muted={props.total === 0 ? "" : undefined}>
		<Show when={props.total > 0}>
			<svg class="lr-alertdot" viewBox="0 0 8 8" aria-hidden="true">
				<circle cx="4" cy="4" r="4" />
			</svg>
		</Show>
		{props.children}
	</span>
);

export default AlertsPill;
