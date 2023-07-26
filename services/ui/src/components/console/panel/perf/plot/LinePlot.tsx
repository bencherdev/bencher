import * as Plot from "@observablehq/plot";
import * as d3 from "d3";
import { createEffect, createSignal } from "solid-js";
import { Range } from "../../../config/types";
import { addTooltips } from "./tooltip";
import {
	JsonPerf,
	JsonAlertStatus,
	JsonPerfAlert,
} from "../../../../../types/bencher";

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

	const get_units = (json_perf: JsonPerf) => {
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
		const json_perf: JsonPerf = props.perf_data();
		// console.log(json_perf);

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
		const alert_arrays = [];
		let metrics_found = false;
		const colors = d3.schemeTableau10;
		const project_slug = json_perf.project.slug;
		json_perf.results.forEach((result, index) => {
			const perf_metrics = result.metrics;
			if (!(Array.isArray(perf_metrics) && props.perf_active[index])) {
				return;
			}

			const line_data = [];
			perf_metrics.forEach((perf_metric) => {
				line_data.push({
					report: perf_metric.report,
					value: perf_metric.metric?.value,
					date_time: new Date(perf_metric.start_time),
					number: perf_metric.version?.number,
					hash: perf_metric.version?.hash,
					iteration: perf_metric.iteration,
					lower_limit: perf_metric.boundary?.lower_limit,
					upper_limit: perf_metric.boundary?.upper_limit,
					alert: perf_metric.alert,
				});
				metrics_found = true;
			});

			const color = colors[index % 10];
			// Line
			plot_arrays.push(
				Plot.line(line_data, {
					x: x_axis,
					y: "value",
					stroke: color,
				}),
			);
			// Dots
			plot_arrays.push(
				Plot.dot(line_data, {
					x: x_axis,
					y: "value",
					stroke: color,
					fill: color,
					title: (datum) => to_title(`${datum.value}`, datum, ""),
				}),
			);

			// Lower Boundary
			const LOWER_LIMIT = "lower_limit";
			const LOWER = "Lower";
			if (props.lower_boundary()) {
				plot_arrays.push(
					Plot.line(line_data, boundary_line(x_axis, LOWER_LIMIT, color)),
				);
				plot_arrays.push(
					Plot.dot(line_data, boundary_dot(x_axis, LOWER_LIMIT, color, LOWER)),
				);
			}
			alert_arrays.push(
				Plot.image(
					line_data,
					alert_image(x_axis, LOWER_LIMIT, LOWER, project_slug),
				),
			);

			// Upper Boundary
			const UPPER_LIMIT = "upper_limit";
			const UPPER = "Upper";
			if (props.upper_boundary()) {
				plot_arrays.push(
					Plot.line(line_data, boundary_line(x_axis, UPPER_LIMIT, color)),
				);
				plot_arrays.push(
					Plot.dot(line_data, boundary_dot(x_axis, UPPER_LIMIT, color, UPPER)),
				);
			}
			alert_arrays.push(
				Plot.image(
					line_data,
					alert_image(x_axis, UPPER_LIMIT, UPPER, project_slug),
				),
			);
		});
		// This allows the alert images to appear on top of the plot lines.
		plot_arrays.push(...alert_arrays);

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

const to_title = (prefix, datum, suffix) =>
	`${prefix}\n${datum.date_time?.toLocaleString(undefined, {
		weekday: "short",
		year: "numeric",
		month: "short",
		day: "2-digit",
		hour: "2-digit",
		hour12: false,
		minute: "2-digit",
		second: "2-digit",
	})}\nVersion Number: ${datum.number}${
		datum.hash ? `\nVersion Hash: ${datum.hash}` : ""
	}\nIteration: ${datum.iteration}${suffix}`;

const boundary_line = (x_axis, y_axis, color) => {
	return {
		x: x_axis,
		y: y_axis,
		stroke: color,
		strokeWidth: 4,
		strokeOpacity: 0.666,
		strokeDasharray: [8],
	};
};

const boundary_dot = (x_axis, y_axis, color, position) => {
	return {
		x: x_axis,
		y: y_axis,
		stroke: color,
		strokeWidth: 4,
		strokeOpacity: 0.666,
		fill: color,
		fillOpacity: 0.666,
		title: (datum) =>
			!is_active(datum.alert) && limit_title(y_axis, position, datum, ""),
	};
};

const limit_title = (y_axis, position, datum, suffix) =>
	to_title(`${position} Limit: ${datum[y_axis]}`, datum, suffix);

const alert_image = (x_axis, y_axis, position, project_slug) => {
	return {
		x: x_axis,
		y: y_axis,
		src: (datum) => is_active(datum.alert) && SIREN_URL,
		width: 18,
		title: (datum) =>
			is_active(datum.alert) &&
			limit_title(y_axis, position, datum, "\nClick to view Alert"),
		href: (datum) =>
			is_active(datum.alert) &&
			`/console/projects/${project_slug}/alerts/${datum.alert.uuid}`,
		target: "_blank",
	};
};

const is_active = (alert: JsonPerfAlert) =>
	alert?.status && alert.status == JsonAlertStatus.Active;

// Source: https://twemoji.twitter.com
// License: https://creativecommons.org/licenses/by/4.0
const SIREN_URL = "https://s3.amazonaws.com/public.bencher.dev/siren.png";

export default LinePlot;
