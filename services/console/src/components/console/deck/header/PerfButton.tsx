import type { Resource } from "solid-js";
import { useNavigate } from "../../../../util/url";
import { JsonLimit, type JsonAlert } from "../../../../types/bencher";
import type { Params } from "astro";

export interface Props {
	params: Params;
	data: Resource<Record<string, any>>;
}

const PerfButton = (props: Props) => {
	const navigate = useNavigate();

	return (
		<button
			class="button is-outlined is-fullwidth"
			title="View Alert"
			onClick={(e) => {
				e.preventDefault();

				const json_alert = props.data() as JsonAlert;
				const perfQuery = {
					metric_kind: json_alert.threshold?.metric_kind?.slug,
					branches: json_alert.threshold?.branch?.uuid,
					testbeds: json_alert.threshold?.testbed?.uuid,
					benchmarks: json_alert.benchmark?.uuid,
					lower_boundary: json_alert.limit === JsonLimit.Lower,
					upper_boundary: json_alert.limit === JsonLimit.Upper,
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
