import { Show } from "solid-js";
import type { GitHubAsk } from "../github";

/** Link GitHub or Reconnect GitHub, as tall as the pills beside it, in a touch target as tall as their line. */
const AskGitHub = (props: { ask: GitHubAsk | undefined }) => (
	<Show when={props.ask}>
		{(ask) => (
			<a style={TARGET} href={ask().href}>
				<span
					class="ui-button"
					style={PILL}
					data-variant="secondary"
					data-size="sm"
				>
					{ask().label}
				</span>
			</a>
		)}
	</Show>
);

export default AskGitHub;

/** The pills' line sets its height: on a phone, a line held tall enough for a finger; elsewhere, the chip's own. */
const TARGET = {
	display: "inline-flex",
	"align-items": "center",
	"min-height": "inherit",
};

const PILL = { "--ui-button-size": "24px" };
