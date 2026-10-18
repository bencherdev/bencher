import Icon from "@bencherdev/ui/Icon";
import { type JSX, Show } from "solid-js";
import type {
	JsonConsoleAuthor,
	JsonReportAlertsCounts,
} from "../../types/bencher";

/** The active alerts, then every alert raised. */
export const Alerts = (props: { alerts: JsonReportAlertsCounts }) => (
	<span class="reports-alerts">
		<Show when={props.alerts.active > 0}>
			<span class="reports-active">
				<span class="reports-dot" aria-hidden="true" />
				{props.alerts.active} active
			</span>{" "}
		</Show>
		<Show
			when={props.alerts.total > 0}
			fallback={<span class="reports-quiet">0</span>}
		>
			<span class="reports-quiet">{props.alerts.total} total</span>
		</Show>
	</span>
);

/** Who sent a report: a project key, else a person. */
export const Author = (props: {
	author: JsonConsoleAuthor;
	children?: JSX.Element;
}) => {
	// Compared by value, so the generated enum stays out of the bundle.
	const key = () => props.author.kind === ("key" as JsonConsoleAuthor["kind"]);
	return (
		<span class="reports-by">
			<span role="img" aria-label={key() ? "Project key" : "User"}>
				<Icon name={key() ? "key" : "user"} />
			</span>
			<span class="rn-name">{props.author.name}</span>
			{props.children}
		</span>
	);
};

/** A count nothing has landed for yet. */
export const Dash = () => <span class="rn-dash">-</span>;
