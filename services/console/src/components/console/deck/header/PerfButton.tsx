import type { Params } from "astro";
import { createMemo, type Resource } from "solid-js";
import type { JsonAlert } from "../../../../types/bencher";
import { alertPerfUrl } from "../hand/card/ReportCard";

export interface Props {
	isConsole: boolean;
	params: Params;
	data: Resource<object>;
}

const PerfButton = (props: Props) => {
	const url = createMemo(() =>
		alertPerfUrl(
			props.isConsole,
			props.params?.project,
			props.data() as JsonAlert,
		),
	);

	return (
		<a
			class="button is-fullwidth"
			type="button"
			title="View Alert"
			href={url()}
		>
			<span class="icon">
				<i class="fas fa-chart-line" />
			</span>
			<span>Perf Plot</span>
		</a>
	);
};
export default PerfButton;
