import * as Plot from "@observablehq/plot";
import * as d3 from "d3";
import { createEffect, createMemo, createSignal } from "solid-js";
import { addTooltips } from "./tooltip";

const LinePlot = (props) => {
	const [is_plotted, set_is_plotted] = createSignal(false);
	const [y_label_area_size, set_y_label_area_size] = createSignal(1000);

	createEffect(() => {
		if (is_plotted()) {
			let y_axis = document.querySelectorAll(
				"svg [aria-label='y-axis'] g.tick text",
			);

			let max_width = 0;
			y_axis.forEach((y_tick) => {
				let width = y_tick.getBoundingClientRect().width;
				max_width = Math.max(max_width, width);
			});

			set_y_label_area_size(max_width * 1.12);
		}
	});

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
				const xy = [x_value, y_value];
				line_data.push(xy);
				metrics_found = true;
			});

			const color = colors[index % 10];
			plot_arrays.push(
				Plot.line(line_data, {
					stroke: color,
					// marker: "circle",
					// title: (x) => {
					// 	console.log(x);
					// 	return "TOOLTIP";
					// },
				}),
			);
			plot_arrays.push(
				Plot.dot(line_data, {
					stroke: color,
					marker: "circle",
					title: (x) => {
						console.log(x);
						return "TOOLTIP";
					},
				}),
			);
		});

		if (metrics_found) {
			return (
				<>
					<div>
						{addTooltips(
							Plot.plot({
								y: {
									grid: true,
									label: `↑ ${units}`,
								},
								marks: plot_arrays,
								width: props.width(),
								nice: true,
								// https://github.com/observablehq/plot/blob/main/README.md#layout-options
								// For simplicity’s sake and for consistent layout across plots, margins are not automatically sized to make room for tick labels; instead, shorten your tick labels or increase the margins as needed.
								marginLeft: y_label_area_size(),
							}),
							{
								fill: "gray",
								opacity: 0.5,
								"stroke-width": "3px",
								stroke: "red",
							},
						)}
					</div>
					<>{set_is_plotted(true)}</>
				</>
			);
		} else {
			return (
				<section class="section">
					<div class="container">
						<div class="content">
							<div>
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
