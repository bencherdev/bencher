import { Match, Show, Switch } from "solid-js";
import { formatDate } from "../plot/format";

/** Bencher records no reason, and a replaced branch head is the only one. */
const SILENCED = "Silenced: the branch head was replaced";

interface AlertActionsProps {
	/** The line, as its row names it. */
	name: string;
	status: "active" | "dismissed" | "silenced";
	/** Dismissed on this page, while the reader stays. */
	justNow: boolean;
	/** When the alert last changed status. */
	modified: number;
	/** Without them the reader cannot change the alert's status. */
	onDismiss?: (() => void) | undefined;
	onReactivate?: (() => void) | undefined;
}

/** An alert row's own controls: Dismiss while active, its status and Reactivate once dismissed, and why when silenced. */
const AlertActions = (props: AlertActionsProps) => {
	// A control that gives way to another, pressed or rolled back, hands it the focus rather than dropping it to the page.
	let pressed = false;
	let shown: HTMLButtonElement | undefined;
	const undo = (button: HTMLButtonElement) => {
		const held = shown !== undefined && document.activeElement === shown;
		shown = button;
		if (pressed || held) {
			pressed = false;
			queueMicrotask(() => button.focus());
		}
	};
	return (
		<Switch>
			<Match when={props.status === "active" && props.onDismiss}>
				{(dismiss) => (
					<button
						ref={undo}
						type="button"
						class="lnk"
						aria-label={`Dismiss the alert on ${props.name}`}
						onClick={() => {
							pressed = true;
							dismiss()();
						}}
					>
						Dismiss
					</button>
				)}
			</Match>
			<Match when={props.status === "dismissed"}>
				<span class="lr-state">
					{props.justNow
						? "Dismissed just now"
						: `Dismissed ${formatDate(props.modified)}`}
				</span>
				<Show when={props.onReactivate}>
					{(reactivate) => (
						<>
							<span class="dotsep" aria-hidden="true">
								·
							</span>
							<button
								ref={undo}
								type="button"
								class="lnk"
								aria-label={`Reactivate the alert on ${props.name}`}
								onClick={() => {
									pressed = true;
									reactivate()();
								}}
							>
								Reactivate
							</button>
						</>
					)}
				</Show>
			</Match>
			<Match when={props.status === "silenced"}>
				<span class="lr-state">{SILENCED}</span>
			</Match>
		</Switch>
	);
};

export default AlertActions;
