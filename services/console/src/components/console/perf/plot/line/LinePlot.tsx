import * as Plot from "@observablehq/plot";
import * as d3 from "d3";
import {
	type Accessor,
	type Resource,
	createEffect,
	createSignal,
	createMemo,
} from "solid-js";
import {
	AlertStatus,
	BoundaryLimit,
	XAxis,
	type Boundary,
	type JsonPerf,
	type JsonPerfAlert,
} from "../../../../../types/bencher";
import { addTooltips } from "./tooltip";
import { BACK_PARAM, encodePath } from "../../../../../util/url";
import { Theme } from "../../../../navbar/theme/theme";
import type { ScaleType } from "@observablehq/plot";

// Source: https://twemoji.twitter.com
// License: https://creativecommons.org/licenses/by/4.0
const WARNING_URL =
	"https://s3.amazonaws.com/public.bencher.dev/perf/warning.png";
const SIREN_URL = "https://s3.amazonaws.com/public.bencher.dev/perf/siren.png";

export interface Props {
	theme: Accessor<Theme>;
	isConsole: boolean;
	perfData: Resource<JsonPerf>;
	x_axis: Accessor<XAxis>;
	lower_value: Accessor<boolean>;
	upper_value: Accessor<boolean>;
	lower_boundary: Accessor<boolean>;
	upper_boundary: Accessor<boolean>;
	perfActive: boolean[];
	width: Accessor<number>;
}

const value_end_position_key = (limit: BoundaryLimit) => {
	switch (limit) {
		case BoundaryLimit.Lower:
			return "lower_value";
		case BoundaryLimit.Upper:
			return "upper_value";
	}
};

const boundary_position_key = (limit: BoundaryLimit) => {
	switch (limit) {
		case BoundaryLimit.Lower:
			return "lower_limit";
		case BoundaryLimit.Upper:
			return "upper_limit";
	}
};

const position_label = (limit: BoundaryLimit) => {
	switch (limit) {
		case BoundaryLimit.Lower:
			return "Lower";
		case BoundaryLimit.Upper:
			return "Upper";
	}
};

const get_units = (json_perf: JsonPerf) => {
	const units = json_perf?.results?.[0]?.measure?.units;
	if (units) {
		return units;
	}
	return "units";
};

const get_x_axis = (x_axis: XAxis): [string, ScaleType, string] => {
	switch (x_axis) {
		case XAxis.DateTime:
			return ["date_time", "time", "Report Date and Time"];
		case XAxis.Version:
			return ["number", "point", "Branch Version Number"];
	}
};

const is_active = (alert: JsonPerfAlert) =>
	alert?.status && alert.status === AlertStatus.Active;

// A boundary is skipped if it is defined but its limit undefined
// This indicates that the the boundary limit could not be calculated for the metric
const boundary_skipped = (
	boundary: undefined | Boundary,
	limit: undefined | number,
) => boundary && !limit;

const LinePlot = (props: Props) => {
	const hoverStyles = createMemo(() => {
		switch (props.theme()) {
			case Theme.Light:
				return {
					fill: "white",
					stroke: "grey",
				};
			case Theme.Dark:
				return {
					fill: "black",
					stroke: "white",
				};
		}
	});

	const [isPlotted, setIsPlotted] = createSignal(false);
	const [y_label_area_size, set_y_label_area_size] = createSignal(512);

	const [x_axis, setRange] = createSignal(props.x_axis());
	const [lower_value, setLowerValue] = createSignal(props.lower_value());
	const [upper_value, setUpperValue] = createSignal(props.upper_value());
	const [lower_boundary, setLowerBoundary] = createSignal(
		props.lower_boundary(),
	);
	const [upper_boundary, setUpperBoundary] = createSignal(
		props.upper_boundary(),
	);

	createEffect(() => {
		if (isPlotted()) {
			const y_axis = document.querySelector(
				"svg [aria-label='y-axis tick label']",
			);
			if (!y_axis) {
				return;
			}
			const width = y_axis.getBoundingClientRect().width;
			set_y_label_area_size(width * 1.12);
		}
		// If any of these change, it is possible for the y-axis labels to change.
		// Therefore, we need to recalculate the plot's `marginLeft` to make sure the new y-axis labels fits.
		if (props.x_axis() !== x_axis()) {
			setRange(props.x_axis());
			setIsPlotted(false);
		} else if (props.lower_value() !== lower_value()) {
			setLowerValue(props.lower_value());
			setIsPlotted(false);
		} else if (props.upper_value() !== upper_value()) {
			setUpperValue(props.upper_value());
			setIsPlotted(false);
		} else if (props.lower_boundary() !== lower_boundary()) {
			setLowerBoundary(props.lower_boundary());
			setIsPlotted(false);
		} else if (props.upper_boundary() !== upper_boundary()) {
			setUpperBoundary(props.upper_boundary());
			setIsPlotted(false);
		}
	});

	const plotted = () => {
		const json_perf = props.perfData();
		// console.log(json_perf);

		if (
			typeof json_perf !== "object" ||
			json_perf === undefined ||
			json_perf === null ||
			!Array.isArray(json_perf.results)
		) {
			return;
		}

		const units = get_units(json_perf);
		const [x_axis_kind, x_axis_scale_type, x_axis_label] = get_x_axis(
			props.x_axis(),
		);

		const plot_arrays = [];
		const warn_arrays = [];
		const alert_arrays = [];
		let metrics_found = false;
		const colors = d3.schemeTableau10;
		const project_slug = json_perf.project.slug;
		for (const [index, result] of json_perf.results.entries()) {
			const perf_metrics = result.metrics;
			if (!(Array.isArray(perf_metrics) && props.perfActive[index])) {
				continue;
			}

			const line_data = [];
			const lower_alert_data = [];
			const upper_alert_data = [];
			const boundary_data = [];
			const skipped_lower_data = [];
			const skipped_upper_data = [];
			// biome-ignore lint/complexity/noForEach: <explanation>
			perf_metrics.forEach((perf_metric) => {
				const datum = {
					report: perf_metric.report,
					metric: perf_metric.metric?.uuid,
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
					threshold: perf_metric.threshold,
				};
				if (perf_metric.alert && is_active(perf_metric.alert)) {
					switch (perf_metric.alert?.limit) {
						case BoundaryLimit.Lower:
							lower_alert_data.push({
								...limit_datum,
								alert: perf_metric.alert,
							});
							break;
						case BoundaryLimit.Upper:
							upper_alert_data.push({
								...limit_datum,
								alert: perf_metric.alert,
							});
							break;
					}
				} else {
					boundary_data.push(limit_datum);
				}

				if (
					boundary_skipped(
						perf_metric.threshold?.model?.lower_boundary,
						perf_metric.boundary?.lower_limit,
					)
				) {
					skipped_lower_data.push({
						date_time: datum.date_time,
						number: datum.number,
						y: perf_metric.metric?.value * 0.9,
						threshold: perf_metric.threshold,
					});
				}
				if (
					boundary_skipped(
						perf_metric.threshold?.model?.upper_boundary,
						perf_metric.boundary?.upper_limit,
					)
				) {
					skipped_upper_data.push({
						date_time: datum.date_time,
						number: datum.number,
						y: perf_metric.metric?.value * 1.1,
						threshold: perf_metric.threshold,
					});
				}

				metrics_found = true;
			});

			const color = colors[index % 10] ?? "7f7f7f";
			// Line
			plot_arrays.push(
				Plot.line(line_data, {
					x: x_axis_kind,
					y: "value",
					stroke: color,
				}),
			);
			// Dots
			plot_arrays.push(
				Plot.dot(line_data, {
					x: x_axis_kind,
					y: "value",
					symbol: "circle",
					stroke: color,
					fill: color,
					title: (datum) =>
						to_title(`${datum.value}`, result, datum, "\nClick to view Metric"),
					href: (datum) => dotUrl(project_slug, props.isConsole, datum),
				}),
			);

			// Lower Value
			if (props.lower_value()) {
				plot_arrays.push(
					Plot.line(
						line_data,
						value_end_line(x_axis_kind, BoundaryLimit.Lower, color),
					),
				);
				plot_arrays.push(
					Plot.dot(
						line_data,
						value_end_dot(x_axis_kind, BoundaryLimit.Lower, color, result),
					),
				);
			}

			// Upper Value
			if (props.upper_value()) {
				plot_arrays.push(
					Plot.line(
						line_data,
						value_end_line(x_axis_kind, BoundaryLimit.Upper, color),
					),
				);
				plot_arrays.push(
					Plot.dot(
						line_data,
						value_end_dot(x_axis_kind, BoundaryLimit.Upper, color, result),
					),
				);
			}

			// Lower Boundary
			if (props.lower_boundary()) {
				plot_arrays.push(
					Plot.line(
						line_data,
						boundary_line(x_axis_kind, BoundaryLimit.Lower, color),
					),
				);
				plot_arrays.push(
					Plot.dot(
						boundary_data,
						boundary_dot(
							x_axis_kind,
							BoundaryLimit.Lower,
							color,
							project_slug,
							result,
							props.isConsole,
						),
					),
				);
				warn_arrays.push(
					Plot.image(
						skipped_lower_data,
						warning_image(x_axis_kind, project_slug, props.isConsole),
					),
				);
			}
			alert_arrays.push(
				Plot.image(
					lower_alert_data,
					alert_image(
						x_axis_kind,
						BoundaryLimit.Lower,
						project_slug,
						result,
						props.isConsole,
					),
				),
			);

			// Upper Boundary
			if (props.upper_boundary()) {
				plot_arrays.push(
					Plot.line(
						line_data,
						boundary_line(x_axis_kind, BoundaryLimit.Upper, color),
					),
				);
				plot_arrays.push(
					Plot.dot(
						boundary_data,
						boundary_dot(
							x_axis_kind,
							BoundaryLimit.Upper,
							color,
							project_slug,
							result,
							props.isConsole,
						),
					),
				);
				warn_arrays.push(
					Plot.image(
						skipped_upper_data,
						warning_image(x_axis_kind, project_slug, props.isConsole),
					),
				);
			}
			alert_arrays.push(
				Plot.image(
					upper_alert_data,
					alert_image(
						x_axis_kind,
						BoundaryLimit.Upper,
						project_slug,
						result,
						props.isConsole,
					),
				),
			);
		}
		// This allows the alert images to appear on top of the plot lines.
		plot_arrays.push(...warn_arrays, ...alert_arrays);

		if (metrics_found) {
			return (
				<>
					<div>
						{addTooltips(
							Plot.plot({
								x: {
									type: x_axis_scale_type,
									grid: true,
									label: `${x_axis_label} ➡`,
									labelOffset: 36,
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
								stroke: "gray",
								opacity: 0.75,
								"stroke-width": "3px",
								fill: "gray",
							},
							hoverStyles(),
						)}
					</div>
					{setIsPlotted(true)}
				</>
			);
		}
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
	};

	return <>{plotted()}</>;
};

const to_title = (prefix, result, datum, suffix) =>
	`${prefix}\n${datum.date_time?.toLocaleString(undefined, {
		weekday: "short",
		year: "numeric",
		month: "short",
		day: "2-digit",
		hour: "2-digit",
		hour12: false,
		minute: "2-digit",
		second: "2-digit",
	})}\nIteration: ${datum.iteration}\nBranch: ${
		result.branch?.name
	}\nVersion Number: ${datum.number}${
		datum.hash ? `\nVersion Hash: ${datum.hash}` : ""
	}\nTestbed: ${result.testbed?.name}\nBenchmark: ${
		result.benchmark?.name
	}\nMeasure: ${result.measure?.name}${suffix}`;

const value_end_line = (
	x_axis: string,
	limit: BoundaryLimit,
	color: string,
) => {
	return {
		x: x_axis,
		y: value_end_position_key(limit),
		stroke: color,
		strokeWidth: 2,
		strokeOpacity: 0.9,
		strokeDasharray: [3],
	};
};

const value_end_dot = (
	x_axis: string,
	limit: BoundaryLimit,
	color: string,
	result: object,
) => {
	return {
		x: x_axis,
		y: value_end_position_key(limit),
		symbol: "diamond",
		stroke: color,
		strokeWidth: 2,
		strokeOpacity: 0.9,
		fill: color,
		fillOpacity: 0.9,
		title: (datum) => value_end_title(limit, result, datum, ""),
	};
};

const boundary_line = (x_axis: string, limit: BoundaryLimit, color) => {
	return {
		x: x_axis,
		y: boundary_position_key(limit),
		stroke: color,
		strokeWidth: 4,
		strokeOpacity: 0.666,
		strokeDasharray: [8],
	};
};

const boundary_dot = (
	x_axis: string,
	limit: BoundaryLimit,
	color: string,
	project_slug: string,
	result: object,
	isConsole: boolean,
) => {
	return {
		x: x_axis,
		y: boundary_position_key(limit),
		symbol: "square",
		stroke: color,
		strokeWidth: 4,
		strokeOpacity: 0.666,
		fill: color,
		fillOpacity: 0.666,
		title: (datum) =>
			limit_title(limit, result, datum, "\nClick to view Threshold"),
		href: (datum) => thresholdUrl(project_slug, isConsole, datum),
	};
};

const warning_image = (
	x_axis: string,
	project_slug: string,
	isConsole: boolean,
) => {
	return {
		x: x_axis,
		y: "y",
		src: WARNING_URL,
		width: 18,
		title: (_datum) =>
			"Boundary Limit was not calculated.\nThis can happen for a couple of reasons:\n- There is not enough data yet (n < 2) (Most Common)\n- All the metric values are the same (variance == 0)\nClick to view Threshold",
		href: (datum) => thresholdUrl(project_slug, isConsole, datum),
	};
};

const dotUrl = (project_slug: string, isConsole: boolean, datum: object) =>
	`${
		isConsole
			? `/console/projects/${project_slug}/metric/${datum.metric}`
			: `/perf/${project_slug}/metrics/${datum.metric}`
	}?${BACK_PARAM}=${encodePath()}`;

const thresholdUrl = (
	project_slug: string,
	isConsole: boolean,
	datum: object,
) =>
	`${
		isConsole
			? `/console/projects/${project_slug}/thresholds/${datum.threshold?.uuid}`
			: `/perf/${project_slug}/thresholds/${datum.threshold?.uuid}`
	}?model=${datum.threshold?.model?.uuid}&${BACK_PARAM}=${encodePath()}`;

const alert_image = (
	x_axis: string,
	limit: BoundaryLimit,
	project_slug: string,
	result: object,
	isConsole: boolean,
) => {
	return {
		x: x_axis,
		y: boundary_position_key(limit),
		src: SIREN_URL,
		width: 18,
		title: (datum) =>
			limit_title(limit, result, datum, "\nClick to view Alert"),
		href: (datum) =>
			`${
				isConsole
					? `/console/projects/${project_slug}/alerts/${datum.alert?.uuid}`
					: `/perf/${project_slug}/alerts/${datum.alert?.uuid}`
			}
			?${BACK_PARAM}=${encodePath()}`,
		// target: "_blank",
	};
};

const value_end_title = (limit: BoundaryLimit, result, datum, suffix) =>
	to_title(
		`${position_label(limit)} Value: ${datum[value_end_position_key(limit)]}`,
		result,
		datum,
		suffix,
	);

const limit_title = (limit: BoundaryLimit, result, datum, suffix) =>
	to_title(
		`${position_label(limit)} Limit: ${datum[boundary_position_key(limit)]}`,
		result,
		datum,
		suffix,
	);

export default LinePlot;
