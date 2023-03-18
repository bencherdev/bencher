import * as Plot from "@observablehq/plot";
import * as d3 from "d3";
import { createSignal } from "solid-js";

const PLOT_ID = "perf_plot";

const LinePlot = (props) => {
	const [max_units, setMaxUnits] = createSignal(1);

	const handleMaxUnits = (value: number) => {
		if (value >= 1.0) {
			value = Math.round(value);
		}
		setMaxUnits(Math.max(max_units(), value.toString().length));
	};

	const get_units = (json_perf) => {
		const units = json_perf?.metric_kind?.units;
		if (units) {
			return units;
		} else {
			return "units";
		}
	};

	const plotted = () => {
		const json_perf = props.perf_data();

		if (
			typeof json_perf !== "object" ||
			json_perf === null ||
			!Array.isArray(json_perf.results)
		) {
			return;
		}

		const units = get_units(json_perf);

		const plot_arrays = [];
		let metrics_found = false;
		const colors = d3.schemeTableau10;
		json_perf.results.forEach((result, index) => {
			const perf_metrics = result.metrics;
			if (!(Array.isArray(perf_metrics) && props.perf_active[index])) {
				return;
			}

			const line_data = [];
			perf_metrics.forEach((perf_metric) => {
				const x_value = new Date(perf_metric.start_time);
				x_value.setSeconds(x_value.getSeconds() + perf_metric.iteration);
				const y_value = perf_metric.metric?.value;
				handleMaxUnits(y_value);
				const xy = [x_value, y_value];
				line_data.push(xy);
				metrics_found = true;
			});

			const color = colors[index % 10];
			plot_arrays.push(Plot.line(line_data, { stroke: color }));
		});

		if (metrics_found) {
			return (
				<div id={PLOT_ID}>
					{Plot.plot({
						y: {
							grid: true,
							label: `↑ ${units}`,
						},
						marks: plot_arrays,
						width: props.width(),
						nice: true,
						// https://github.com/observablehq/plot/blob/main/README.md#layout-options
						// For simplicity’s sake and for consistent layout across plots, margins are not automatically sized to make room for tick labels; instead, shorten your tick labels or increase the margins as needed.
						marginLeft: max_units() * 10 + 10,
					})}
				</div>
			);
		} else {
			return (
				<section class="section">
					<div class="container">
						<div class="content">
							<div id={PLOT_ID}>
								<h3 class="title is-3">No data found</h3>
								<h4 class="subtitle is-4">{new Date(Date.now()).toString()}</h4>
							</div>
						</div>
					</div>
				</section>
			);
		}
	};

	return <>{plotted()}</>;
};

export default LinePlot;
