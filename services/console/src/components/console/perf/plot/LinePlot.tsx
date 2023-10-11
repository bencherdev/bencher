import * as Plot from "@observablehq/plot";
import * as d3 from "d3";
import { type Accessor, createEffect, createSignal } from "solid-js";
import { addTooltips } from "./tooltip";
import {
	AlertStatus,
	type Boundary,
	type JsonPerf,
	type JsonPerfAlert,
} from "../../../../types/bencher";
import { PerfRange } from "../../../../config/types";

// Source: https://twemoji.twitter.com
// License: https://creativecommons.org/licenses/by/4.0
const WARNING_URL = "https://s3.amazonaws.com/public.bencher.dev/warning.png";
const SIREN_URL = "https://s3.amazonaws.com/public.bencher.dev/siren.png";

export interface Props {
	perfData: Accessor<JsonPerf>;
	range: Accessor<PerfRange>;
	lower_value: Accessor<boolean>;
	upper_value: Accessor<boolean>;
	lower_boundary: Accessor<boolean>;
	upper_boundary: Accessor<boolean>;
	perfActive: boolean[];
	width: Accessor<number>;
}

enum Position {
	Lower,
	Upper,
}

const value_end_position_key = (position: Position) => {
	switch (position) {
		case Position.Lower:
			return "lower_value";
		case Position.Upper:
			return "upper_value";
	}
};

const boundary_position_key = (position: Position) => {
	switch (position) {
		case Position.Lower:
			return "lower_limit";
		case Position.Upper:
			return "upper_limit";
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

const get_units = (json_perf: JsonPerf) => {
	const units = json_perf?.metric_kind?.units;
	if (units) {
		return units;
	} else {
		return "units";
	}
};

const get_x_axis = (range: PerfRange): [string, string] => {
	switch (range) {
		case PerfRange.DATE_TIME:
			return ["date_time", "Report Date and Time"];
		case PerfRange.VERSION:
			return ["number", "Branch Version Number"];
	}
};

const is_active = (alert: JsonPerfAlert) =>
	alert?.status && alert.status == AlertStatus.Active;

// A boundary is skipped if it is defined but its limit undefined
// This indicates that the the boundary limit could not be calculated for the metric
const boundary_skipped = (
	boundary: undefined | Boundary,
	limit: undefined | number,
) => boundary && !limit;

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
				const width = y_tick.getBoundingClientRect().width;
				max_width = Math.max(max_width, width);
			});

			set_y_label_area_size(max_width * 1.12);
		}
	});

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
		const [x_axis, x_axis_label] = get_x_axis(props.range());

		const plot_arrays = [];
		const warn_arrays = [];
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
			const alert_data = [];
			const boundary_data = [];
			const skipped_lower_data = [];
			const skipped_upper_data = [];
			perf_metrics.forEach((perf_metric) => {
				const datum = {
					report: perf_metric.report,
					value: perf_metric.metric?.value,
					lower_value: perf_metric.metric?.lower_value,
					upper_value: perf_metric.metric?.upper_value,
					date_time: new Date(perf_metric.start_time),
					number: perf_metric.version?.number,
					hash: perf_metric.version?.hash,
					iteration: perf_metric.iteration,
					lower_limit: perf_metric.boundary?.lower_limit,
					upper_limit: perf_metric.boundary?.upper_limit,
				};
				line_data.push(datum);

				const limit_datum = {
					date_time: datum.date_time,
					number: datum.number,
					hash: datum.hash,
					iteration: datum.iteration,
					lower_limit: datum.lower_limit,
					upper_limit: datum.upper_limit,
				};
				if (perf_metric.alert && is_active(perf_metric.alert)) {
					alert_data.push({ ...limit_datum, alert: perf_metric.alert });
				} else {
					boundary_data.push(limit_datum);
				}

				if (
					boundary_skipped(
						perf_metric.threshold?.statistic?.lower_boundary,
						perf_metric.boundary?.lower_limit,
					)
				) {
					skipped_lower_data.push({
						date_time: datum.date_time,
						number: datum.number,
						y: perf_metric.metric?.value * 0.9,
					});
				}
				if (
					boundary_skipped(
						perf_metric.threshold?.statistic?.upper_boundary,
						perf_metric.boundary?.upper_limit,
					)
				) {
					skipped_upper_data.push({
						date_time: datum.date_time,
						number: datum.number,
						y: perf_metric.metric?.value * 1.1,
					});
				}

				metrics_found = true;
			});

			const color = colors[index % 10] ?? "7f7f7f";
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

			// Lower Value
			if (props.lower_value()) {
				plot_arrays.push(
					Plot.line(line_data, value_end_line(x_axis, Position.Lower, color)),
				);
				plot_arrays.push(
					Plot.dot(line_data, value_end_dot(x_axis, Position.Lower, color)),
				);
			}

			// Upper Value
			if (props.upper_value()) {
				plot_arrays.push(
					Plot.line(line_data, value_end_line(x_axis, Position.Upper, color)),
				);
				plot_arrays.push(
					Plot.dot(line_data, value_end_dot(x_axis, Position.Upper, color)),
				);
			}

			// Lower Boundary
			if (props.lower_boundary()) {
				plot_arrays.push(
					Plot.line(line_data, boundary_line(x_axis, Position.Lower, color)),
				);
				plot_arrays.push(
					Plot.dot(boundary_data, boundary_dot(x_axis, Position.Lower, color)),
				);
				warn_arrays.push(Plot.image(skipped_lower_data, warning_image(x_axis)));
			}
			alert_arrays.push(
				Plot.image(
					alert_data,
					alert_image(x_axis, Position.Lower, project_slug),
				),
			);

			// Upper Boundary
			if (props.upper_boundary()) {
				plot_arrays.push(
					Plot.line(line_data, boundary_line(x_axis, Position.Upper, color)),
				);
				plot_arrays.push(
					Plot.dot(boundary_data, boundary_dot(x_axis, Position.Upper, color)),
				);
				warn_arrays.push(Plot.image(skipped_upper_data, warning_image(x_axis)));
			}
			alert_arrays.push(
				Plot.image(
					alert_data,
					alert_image(x_axis, Position.Upper, project_slug),
				),
			);
		});
		// This allows the alert images to appear on top of the plot lines.
		plot_arrays.push(...warn_arrays, ...alert_arrays);

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

const value_end_line = (x_axis: string, position: Position, color: string) => {
	return {
		x: x_axis,
		y: value_end_position_key(position),
		stroke: color,
		strokeWidth: 2,
		strokeOpacity: 0.9,
		strokeDasharray: [3],
	};
};

const value_end_dot = (x_axis: string, position: Position, color: string) => {
	return {
		x: x_axis,
		y: value_end_position_key(position),
		stroke: color,
		strokeWidth: 2,
		strokeOpacity: 0.9,
		fill: color,
		fillOpacity: 0.9,
		title: (datum) => value_end_title(position, datum, ""),
		// TODO enable this when there is an endpoint for getting a historical threshold statistic
		// That is, the statistic displayed needs to be historical, not current.
		// Just like with the Alerts.
		// href: (datum) =>
		// 	!is_active(datum.alert) &&
		// 	`/console/projects/${project_slug}/thresholds/${datum.threshold}`,
		// target: "_blank",
	};
};

const boundary_line = (x_axis: string, position: Position, color) => {
	return {
		x: x_axis,
		y: boundary_position_key(position),
		stroke: color,
		strokeWidth: 4,
		strokeOpacity: 0.666,
		strokeDasharray: [8],
	};
};

const boundary_dot = (x_axis: string, position: Position, color: string) => {
	return {
		x: x_axis,
		y: boundary_position_key(position),
		stroke: color,
		strokeWidth: 4,
		strokeOpacity: 0.666,
		fill: color,
		fillOpacity: 0.666,
		title: (datum) => limit_title(position, datum, ""),
		// TODO enable this when there is an endpoint for getting a historical threshold statistic
		// That is, the statistic displayed needs to be historical, not current.
		// Just like with the Alerts.
		// href: (datum) =>
		// 	!is_active(datum.alert) &&
		// 	`/console/projects/${project_slug}/thresholds/${datum.threshold}`,
		// target: "_blank",
	};
};

const warning_image = (x_axis: string) => {
	return {
		x: x_axis,
		y: "y",
		src: WARNING_URL,
		width: 18,
		title: (_datum) =>
			"Boundary Limit was not calculated.\nThis can happen for a couple of reasons:\n- There is not enough data yet (n < 2) (Most Common)\n- All the metric values are the same (variance == 0)",
	};
};

const alert_image = (
	x_axis: string,
	position: Position,
	project_slug: string,
) => {
	return {
		x: x_axis,
		y: boundary_position_key(position),
		src: SIREN_URL,
		width: 18,
		title: (datum) => limit_title(position, datum, "\nClick to view Alert"),
		href: (datum) =>
			`/console/projects/${project_slug}/alerts/${datum.alert?.uuid}`,
		target: "_blank",
	};
};

const value_end_title = (position: Position, datum, suffix) =>
	to_title(
		`${position_label(position)} Value: ${
			datum[value_end_position_key(position)]
		}`,
		datum,
		suffix,
	);

const limit_title = (position: Position, datum, suffix) =>
	to_title(
		`${position_label(position)} Limit: ${
			datum[boundary_position_key(position)]
		}`,
		datum,
		suffix,
	);

export default LinePlot;
