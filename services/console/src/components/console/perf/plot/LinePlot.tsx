import * as Plot from "@observablehq/plot";
import * as d3 from "d3";
import { Accessor, createEffect, createSignal } from "solid-js";
import { addTooltips } from "./tooltip";
import {
	JsonAlertStatus,
	type Boundary,
	type JsonPerf,
	type JsonPerfAlert,
} from "../../../../types/bencher";
import { PerfRange } from "../../../../config/types";

export interface Props {
	perfData: Accessor<JsonPerf>;
	range: Accessor<PerfRange>;
	lower_boundary: Accessor<boolean>;
	upper_boundary: Accessor<boolean>;
	perfActive: boolean[];
	width: Accessor<number>;
}

enum Position {
	Lower,
	Upper,
}

const position_key = (position: Position) => {
	switch (position) {
		case Position.Lower:
			return "lower_limit";
		case Position.Upper:
			return "upper_limit";
	}
};

const position_skipped_key = (position: Position) => {
	switch (position) {
		case Position.Lower:
			return "lower_boundary_skipped";
		case Position.Upper:
			return "upper_boundary_skipped";
	}
};

const position_label = (position: Position) => {
	switch (position) {
		case Position.Lower:
			return "Lower";
		case Position.Upper:
			return "Upper";
	}
};

const LinePlot = (props: Props) => {
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
			case PerfRange.DATE_TIME:
				return ["date_time", "Report Date and Time"];
			case PerfRange.VERSION:
				return ["number", "Branch Version Number"];
		}
	};

	const plotted = () => {
		const json_perf: JsonPerf = props.perfData();
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
			if (!(Array.isArray(perf_metrics) && props.perfActive[index])) {
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
					threshold: perf_metric.threshold?.uuid,
					lower_limit: perf_metric.boundary?.lower_limit,
					lower_boundary_skipped: boundary_skipped(
						perf_metric.threshold?.statistic?.lower_boundary,
						perf_metric.boundary?.lower_limit,
					)
						? perf_metric.metric?.value * 0.9
						: null,
					upper_limit: perf_metric.boundary?.upper_limit,
					upper_boundary_skipped: boundary_skipped(
						perf_metric.threshold?.statistic?.upper_boundary,
						perf_metric.boundary?.upper_limit,
					)
						? perf_metric.metric?.value * 1.1
						: null,
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
			if (props.lower_boundary()) {
				plot_arrays.push(
					Plot.line(line_data, boundary_line(x_axis, Position.Lower, color)),
				);
				plot_arrays.push(
					Plot.dot(
						line_data,
						boundary_dot(x_axis, Position.Lower, color, project_slug),
					),
				);
				plot_arrays.push(
					Plot.image(line_data, warning_image(x_axis, Position.Lower)),
				);
			}
			alert_arrays.push(
				Plot.image(
					line_data,
					alert_image(x_axis, Position.Lower, project_slug),
				),
			);

			// Upper Boundary
			if (props.upper_boundary()) {
				plot_arrays.push(
					Plot.line(line_data, boundary_line(x_axis, Position.Upper, color)),
				);
				plot_arrays.push(
					Plot.dot(
						line_data,
						boundary_dot(x_axis, Position.Upper, color, project_slug),
					),
				);
				plot_arrays.push(
					Plot.image(line_data, warning_image(x_axis, Position.Upper)),
				);
			}
			alert_arrays.push(
				Plot.image(
					line_data,
					alert_image(x_axis, Position.Upper, project_slug),
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

// A boundary is skipped if it is defined but its limit undefined
// This indicates that the the boundary limit could not be calculated for the metric
const boundary_skipped = (
	boundary: undefined | Boundary,
	limit: undefined | number,
) => boundary && !limit;

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

const boundary_line = (x_axis, position: Position, color) => {
	return {
		x: x_axis,
		y: position_key(position),
		stroke: color,
		strokeWidth: 4,
		strokeOpacity: 0.666,
		strokeDasharray: [8],
	};
};

const boundary_dot = (x_axis, position: Position, color, project_slug) => {
	return {
		x: x_axis,
		y: position_key(position),
		stroke: color,
		strokeWidth: 4,
		strokeOpacity: 0.666,
		fill: color,
		fillOpacity: 0.666,
		title: (datum) =>
			!is_active(datum.alert) && limit_title(position, datum, ""),
		// TODO enable this when there is an endpoint for getting a historical threshold statistic
		// That is, the statistic displayed needs to be historical, not current.
		// Just like with the Alerts.
		// href: (datum) =>
		// 	!is_active(datum.alert) &&
		// 	`/console/projects/${project_slug}/thresholds/${datum.threshold}`,
		// target: "_blank",
	};
};

const warning_image = (x_axis, position: Position) => {
	return {
		x: x_axis,
		y: position_skipped_key(position),
		src: (datum) => WARNING_URL,
		width: 18,
		title: (datum) =>
			is_skipped(position, datum) &&
			"Boundary Limit was not calculated.\nThis can happen for a couple of reasons:\n- There is not enough data yet (n < 2) (Most Common)\n- All the metric values are the same (variance == 0)",
	};
};

const skipped_offset = (position: Position, datum) => {
	if (!is_skipped(position, datum)) {
		return;
	}
	switch (position) {
		case Position.Lower:
			return datum.value * 0.9;
		case Position.Upper:
			return datum.value * 1.1;
	}
};
const is_skipped = (position: Position, datum) =>
	(position === Position.Lower && datum.lower_boundary_skipped) ||
	(position === Position.Upper && datum.upper_boundary_skipped);

const alert_image = (x_axis, position: Position, project_slug) => {
	return {
		x: x_axis,
		y: position_key(position),
		src: (datum) => is_active(datum.alert) && SIREN_URL,
		width: 18,
		title: (datum) =>
			is_active(datum.alert) &&
			limit_title(position, datum, "\nClick to view Alert"),
		href: (datum) =>
			is_active(datum.alert) &&
			`/console/projects/${project_slug}/alerts/${datum.alert?.uuid}`,
		target: "_blank",
	};
};

const is_active = (alert: JsonPerfAlert) =>
	alert?.status && alert.status == JsonAlertStatus.Active;

const limit_title = (position: Position, datum, suffix) =>
	to_title(
		`${position_label(position)} Limit: ${datum[position_key(position)]}`,
		datum,
		suffix,
	);

// Source: https://twemoji.twitter.com
// License: https://creativecommons.org/licenses/by/4.0
const WARNING_URL = "https://s3.amazonaws.com/public.bencher.dev/warning.png";
const SIREN_URL = "https://s3.amazonaws.com/public.bencher.dev/siren.png";

export default LinePlot;
