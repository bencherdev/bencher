import { createMemo, type Resource, Show } from "solid-js";
import { fmtDate } from "../../../../util/convert";
import { PubResourceKind } from "../../../perf/util";

export interface Props {
	resource: PubResourceKind;
	data: Resource<object>;
}

const ArchivedButton = (props: Props) => {
	const archived = createMemo(() => {
		switch (props.resource) {
			case PubResourceKind.Report:
			case PubResourceKind.Metric:
			case PubResourceKind.Threshold:
				if (props.data()?.branch?.archived) {
					return props.data()?.branch?.archived;
					// biome-ignore lint/style/noUselessElse: clearer with else if
				} else if (props.data()?.testbed?.archived) {
					return props.data()?.testbed?.archived;
				}
				return;
			case PubResourceKind.Branch:
			case PubResourceKind.Testbed:
			case PubResourceKind.Benchmark:
			case PubResourceKind.Measure:
				return props.data()?.archived;
			case PubResourceKind.Alert:
				if (props.data()?.threshold?.branch?.archived) {
					return props.data()?.threshold?.branch?.archived;
					// biome-ignore lint/style/noUselessElse: clearer with else if
				} else if (props.data()?.threshold?.testbed?.archived) {
					return props.data()?.threshold?.testbed?.archived;
				}
				return;
		}
	});

	return (
		<Show when={archived()}>
			<div class="columns">
				<div class="column">
					<div class="notification is-warning">
						<p>
							This {props.resource ?? "resource"} was archived on{" "}
							{fmtDate(archived())}
						</p>
					</div>
				</div>
			</div>
		</Show>
	);
};

export default ArchivedButton;
