import * as Plot from "@observablehq/plot";
import * as d3 from "d3";
import { createEffect, createMemo, createSignal } from "solid-js";
import { Range } from "../../../config/types";
import { addTooltips } from "./tooltip";

const LinePlot = (props) => {
	const [is_plotted, set_is_plotted] = createSignal(false);
	const [y_label_area_size, set_y_label_area_size] = createSignal(512);

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

	const get_x_axis = () => {
		switch (props.range()) {
			case Range.DATE_TIME:
				return ["date_time", "Report Date and Time"];
			case Range.VERSION:
				return ["number", "Branch Version Number"];
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
		const [x_axis, x_axis_label] = get_x_axis();

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
				line_data.push({
					value: perf_metric.metric?.value,
					date_time: new Date(perf_metric.start_time),
					number: perf_metric.version?.number,
					hash: perf_metric.version?.hash,
					iteration: perf_metric.iteration,
				});
				metrics_found = true;
			});

			const color = colors[index % 10];
			plot_arrays.push(
				Plot.line(line_data, {
					x: x_axis,
					y: "value",
					stroke: color,
				}),
			);
			plot_arrays.push(
				Plot.dot(line_data, {
					x: x_axis,
					y: "value",
					stroke: color,
					fill: color,
					title: (dot) =>
						`${dot.value}\n${dot.date_time?.toLocaleString(undefined, {
							weekday: "short",
							year: "numeric",
							month: "short",
							day: "2-digit",
							hour: "2-digit",
							hour12: false,
							minute: "2-digit",
							second: "2-digit",
						})}\nVersion Number: ${dot.number}${
							dot.hash ? `\nVersion Hash: ${dot.hash}` : ""
						}\nIteration: ${dot.iteration}`,
				}),
			);
		});

		if (metrics_found) {
			return (
				<>
					<div>
						{addTooltips(
							Plot.plot({
								x: {
									grid: true,
									label: `${x_axis_label} ➡`,
								},
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
								stroke: "#ed6704",
								opacity: 0.5,
								"stroke-width": "3px",
								fill: "gray",
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
