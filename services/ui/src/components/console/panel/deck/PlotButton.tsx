import { useNavigate } from "solid-app-router";
import { JsonAlert, JsonLimit } from "../../../../types/bencher";

const PlotButton = (props) => {
	const navigate = useNavigate();

	return (
		<button
			class="button is-outlined is-fullwidth"
			title="Dismiss alert"
			onClick={(e) => {
				e.preventDefault();

				const json_alert: JsonAlert = props.data();
				const perf_query = {
					metric_kind: json_alert.threshold?.metric_kind?.slug,
					branches: json_alert.threshold?.branch?.uuid,
					testbeds: json_alert.threshold?.testbed?.uuid,
					benchmarks: json_alert.benchmark?.uuid,
					lower_boundary: null,
					upper_boundary: null,
				};
				switch (json_alert.limit) {
					case JsonLimit.Lower:
						perf_query.lower_boundary = true;
					case JsonLimit.Upper:
						perf_query.upper_boundary = true;
				}

				const search_params = new URLSearchParams();
				for (const [key, value] of Object.entries(perf_query)) {
					if (value) {
						search_params.set(key, value);
					}
				}
				const url = `/console/projects/${
					props.path_params.project_slug
				}/perf?${search_params.toString()}`;
				console.log(url);
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
export default PlotButton;
