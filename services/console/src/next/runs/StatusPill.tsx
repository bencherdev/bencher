import Icon from "@bencherdev/ui/Icon";
import { type JSX, Show } from "solid-js";
import { type JobKind, LABELS, type Status } from "./status";

/** A finished run or report's time, else its runner job's state as a pill. */
const StatusPill = (props: { status: Status }) => (
	<Show when={props.status.kind} fallback={props.status.time}>
		{(kind) => (
			<span class="rn-st" data-status={kind()}>
				{MARKS[kind()]()}
				{LABELS[kind()]}
				<Show when={props.status.time}>
					{(time) => (
						<>
							{" "}
							<b>{time()}</b>
						</>
					)}
				</Show>
			</span>
		)}
	</Show>
);

const MARKS: Record<JobKind, () => JSX.Element> = {
	queued: () => <span class="rn-ring" aria-hidden="true" />,
	running: () => <span class="rn-pulse" aria-hidden="true" />,
	failed: () => <Icon name="warning" />,
	canceled: () => <Icon name="close" />,
};

export default StatusPill;
