import type { Params } from "astro";
import type { Resource } from "solid-js";
import { BoundaryLimit, type JsonAlert } from "../../../../types/bencher";
import { dateTimeMillis } from "../../../../util/convert";
import { useNavigate } from "../../../../util/url";

export interface Props {
	params: Params;
	data: Resource<Record<string, any>>;
}

// 30 days
const DEFAULT_ALERT_HISTORY = 30 * 24 * 60 * 60 * 1000;

const PerfButton = (props: Props) => {
	const navigate = useNavigate();

	return (
		<button
			class="button is-outlined is-fullwidth"
			title="View Alert"
			onClick={(e) => {
				e.preventDefault();

				const json_alert = props.data() as JsonAlert;
				const start_time = dateTimeMillis(json_alert?.created);
				const perfQuery = {
					metric_kinds: json_alert.threshold?.metric_kind?.uuid,
					branches: json_alert.threshold?.branch?.uuid,
					testbeds: json_alert.threshold?.testbed?.uuid,
					benchmarks: json_alert.benchmark?.uuid,
					lower_boundary: json_alert.limit === BoundaryLimit.Lower,
					upper_boundary: json_alert.limit === BoundaryLimit.Upper,
					start_time: start_time ? start_time - DEFAULT_ALERT_HISTORY : null,
					end_time: dateTimeMillis(json_alert?.modified),
				};

				const searchParams = new URLSearchParams();
				for (const [key, value] of Object.entries(perfQuery)) {
					if (value) {
						searchParams.set(key, value.toString());
					}
				}
				const url = `/console/projects/${
					props.params.project
				}/perf?${searchParams.toString()}`;
				navigate(url);
			}}
		>
			<span class="icon">
				<i class="fas fa-chart-line" aria-hidden="true" />
			</span>
			<span>Perf Plot</span>
		</button>
	);
};
export default PerfButton;
