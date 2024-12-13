import * as Plot from "@observablehq/plot";
import type { ScaleType } from "@observablehq/plot";
import * as d3 from "d3";
import {
	type Accessor,
	type Resource,
	Show,
	createEffect,
	createMemo,
	createSignal,
} from "solid-js";
import { resourcePath } from "../../../../../config/util";
import {
	AlertStatus,
	type Boundary,
	BoundaryLimit,
	type JsonPerf,
	type JsonPerfAlert,
	type JsonPerfMetrics,
	XAxis,
} from "../../../../../types/bencher";
import { BACK_PARAM, encodePath } from "../../../../../util/url";
import { Theme } from "../../../../navbar/theme/theme";
import { addTooltips } from "./tooltip";

// Source: https://twemoji.twitter.com
// License: https://creativecommons.org/licenses/by/4.0
const WARNING_URL =
	"https://s3.amazonaws.com/public.bencher.dev/perf/warning.png";
const SIREN_URL = "https://s3.amazonaws.com/public.bencher.dev/perf/siren.png";

export interface Props {
	theme: Accessor<Theme>;
	isConsole: boolean;
	plotId: string | undefined;
	perfData: Resource<JsonPerf>;
	x_axis: Accessor<XAxis>;
	lower_value: Accessor<boolean>;
	upper_value: Accessor<boolean>;
	lower_boundary: Accessor<boolean>;
	upper_boundary: Accessor<boolean>;
	perfActive: boolean[];
	width: Accessor<number>;
}

const LinePlot = (props: Props) => {
	const [isPlotted, setIsPlotted] = createSignal(false);
	const handleIsPlotted = () => setIsPlotted(true);
	const [y_label_area_size, set_y_label_area_size] = createSignal(512);
	const tagId = createMemo(() =>
		props.plotId ? `plot-${props.plotId}` : "plot",
	);

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
			const tagIdEscaped = CSS.escape(tagId());
			const y_axis = document.querySelector(
				`#${tagIdEscaped} svg [aria-label='y-axis tick label']`,
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

	const linePlot = createMemo(() => line_plot(props));

	return (
		<Show
			when={linePlot().metrics_found}
			fallback={
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
			}
		>
			<>
				<div id={tagId()}>
					{addTooltips(
						Plot.plot({
							x: {
								type: linePlot().x_axis_scale_type,
								grid: true,
								label: `${linePlot().x_axis_label} ➡`,
								labelOffset: 36,
							},
							y: {
								grid: true,
								label: `↑ ${linePlot().units}`,
							},
							marks: linePlot().marks,
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
						linePlot().hoverStyles,
					)}
				</div>
				{handleIsPlotted()}
			</>
		</Show>
	);
};

const line_plot = (props: Props) => {
	const json_perf = props.perfData();
	// console.log(json_perf);

	if (
		typeof json_perf !== "object" ||
		json_perf === undefined ||
		json_perf === null ||
		!Array.isArray(json_perf.results)
	) {
		return {
			metrics_found: false,
		};
	}

	let metrics_found = false;
	const raw_data = json_perf.results.map((result, index) => {
		const data = perf_result(result, index, props.perfActive);
		if ((data?.line_data?.length ?? 0) > 0) {
			metrics_found = true;
		}
		return data;
	});

	const raw_units = get_units(json_perf);
	const [x_axis_kind, x_axis_scale_type, x_axis_label] = get_x_axis(
		props.x_axis(),
	);
	const [data, units] = scale_data(raw_data, raw_units, {
		lower_value: props.lower_value,
		upper_value: props.upper_value,
		lower_boundary: props.lower_boundary,
		upper_boundary: props.upper_boundary,
	});

	const marks = plot_marks(data, {
		project_slug: json_perf.project.slug,
		isConsole: props.isConsole,
		plotId: props.plotId,
		lower_value: props.lower_value,
		upper_value: props.upper_value,
		lower_boundary: props.lower_boundary,
		upper_boundary: props.upper_boundary,
		x_axis_kind,
	});

	return {
		metrics_found,
		x_axis_scale_type,
		x_axis_label,
		units,
		marks,
		hoverStyles: hover_styles(props.theme()),
	};
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

const scale_data = (
	raw_data: object[],
	raw_units: string,
	props: {
		lower_value: Accessor<boolean>;
		upper_value: Accessor<boolean>;
		lower_boundary: Accessor<boolean>;
		upper_boundary: Accessor<boolean>;
	},
) => {
	const MAX = Number.MAX_SAFE_INTEGER;

	const min = raw_data.reduce(
		(min, data) =>
			Math.min(
				min,
				// The primary metric series
				Math.min(...data.line_data.map((datum) => datum.value ?? MAX)),
				// The lower value series, if active
				props.lower_value()
					? Math.min(...data.line_data.map((datum) => datum.lower_value ?? MAX))
					: MAX,
				// The upper value series, if active
				props.upper_value()
					? Math.min(...data.line_data.map((datum) => datum.upper_value ?? MAX))
					: MAX,
				// The lower boundary series, if active
				props.lower_boundary()
					? Math.min(
							...data.line_data.map((datum) => datum.lower_limit ?? MAX),
							...data.skipped_lower_data.map((datum) => datum.y ?? MAX),
						)
					: MAX,
				// The upper boundary series, if active
				props.upper_boundary()
					? Math.min(
							...data.line_data.map((datum) => datum.upper_limit ?? MAX),
							...data.skipped_upper_data.map((datum) => datum.y ?? MAX),
						)
					: MAX,
				// The lower alert series
				Math.min(
					...data.lower_alert_data.map((datum) => datum.lower_limit ?? MAX),
				),
				// The upper alert series
				Math.min(
					...data.upper_alert_data.map((datum) => datum.upper_limit ?? MAX),
				),
			),
		MAX,
	);
	console.log(min);

	const scale = (() => {
		if (raw_units === "nanoseconds (ns)") {
			if (min > Time.Hours) {
				return Time.Hours;
			}
			if (min > Time.Minutes) {
				return Time.Minutes;
			}
			if (min > Time.Seconds) {
				return Time.Seconds;
			}
			if (min > Time.Millis) {
				return Time.Millis;
			}
			if (min > Time.Micros) {
				return Time.Micros;
			}
			return Time.Nanos;
		}
		if (min > OneE3.Fifteen) {
			return OneE3.Fifteen;
		}
		if (min > OneE3.Twelve) {
			return OneE3.Twelve;
		}
		if (min > OneE3.Nine) {
			return OneE3.Nine;
		}
		if (min > OneE3.Six) {
			return OneE3.Six;
		}
		if (min > OneE3.Three) {
			return OneE3.Three;
		}
		return OneE3.One;
	})();

	// return {
	// 	index,
	// 	result,
	// 	line_data,
	// 	lower_alert_data,
	// 	upper_alert_data,
	// 	boundary_data,
	// 	skipped_lower_data,
	// 	skipped_upper_data,
	// };

	const scaled_data = raw_data.map((data) => {
		data.line_data = data.line_data?.map((datum) => {
			datum.value = datum.value / scale;
			if (datum.lower_value !== undefined && datum.lower_value !== null) {
				datum.lower_value = datum.lower_value / scale;
			}
			if (datum.upper_value !== undefined && datum.upper_value !== null) {
				datum.upper_value = datum.upper_value / scale;
			}
			if (datum.lower_limit !== undefined && datum.lower_limit !== null) {
				datum.lower_limit = datum.lower_limit / scale;
			}
			if (datum.upper_limit !== undefined && datum.upper_limit !== null) {
				datum.upper_limit = datum.upper_limit / scale;
			}
			return datum;
		});
		data.lower_alert_data = data.lower_alert_data?.map((datum) => {
			if (datum.lower_limit !== undefined && datum.lower_limit !== null) {
				datum.lower_limit = datum.lower_limit / scale;
			}
			if (datum.upper_limit !== undefined && datum.upper_limit !== null) {
				datum.upper_limit = datum.upper_limit / scale;
			}
			return datum;
		});
		data.upper_alert_data = data.upper_alert_data?.map((datum) => {
			if (datum.lower_limit !== undefined && datum.lower_limit !== null) {
				datum.lower_limit = datum.lower_limit / scale;
			}
			if (datum.upper_limit !== undefined && datum.upper_limit !== null) {
				datum.upper_limit = datum.upper_limit / scale;
			}
			return datum;
		});
		data.boundary_data = data.boundary_data?.map((datum) => {
			if (datum.lower_limit !== undefined && datum.lower_limit !== null) {
				datum.lower_limit = datum.lower_limit / scale;
			}
			if (datum.upper_limit !== undefined && datum.upper_limit !== null) {
				datum.upper_limit = datum.upper_limit / scale;
			}
			return datum;
		});
		data.skipped_lower_data = data.skipped_lower_data?.map((datum) => {
			datum.y = datum.y / scale;
			return datum;
		});
		data.skipped_upper_data = data.skipped_upper_data?.map((datum) => {
			datum.y = datum.y / scale;
			return datum;
		});
		return data;
	});

	return [scaled_data, raw_units];
};

enum Time {
	Nanos = 1,
	Micros = 1_000,
	Millis = 1_000_000,
	Seconds = 1_000_000_000,
	Minutes = 60_000_000_000,
	Hours = 3_600_000_000_000,
}

enum OneE3 {
	One = 1,
	Three = 1_000,
	Six = 1_000_000,
	Nine = 1_000_000_000,
	Twelve = 1_000_000_000_000,
	Fifteen = 1_000_000_000_000_000,
}

const hover_styles = (theme: Theme) => {
	switch (theme) {
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
};

const perf_result = (
	result: JsonPerfMetrics,
	index: number,
	perfActive: boolean[],
) => {
	const perf_metrics = result.metrics;
	if (!(Array.isArray(perf_metrics) && perfActive[index])) {
		return null;
	}

	const line_data = [];
	const lower_alert_data = [];
	const upper_alert_data = [];
	const boundary_data = [];
	const skipped_lower_data = [];
	const skipped_upper_data = [];

	for (const perf_metric of perf_metrics) {
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
			// These values do not get scaled and are used by the tooltip
			raw: {
				value: perf_metric.metric?.value,
				lower_value: perf_metric.metric?.lower_value,
				upper_value: perf_metric.metric?.upper_value,
				lower_limit: perf_metric.boundary?.lower_limit,
				upper_limit: perf_metric.boundary?.upper_limit,
			},
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
			// These values do not get scaled and are used by the tooltip
			raw: {
				lower_limit: datum.lower_limit,
				upper_limit: datum.upper_limit,
			},
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
	}

	return {
		index,
		result,
		line_data,
		lower_alert_data,
		upper_alert_data,
		boundary_data,
		skipped_lower_data,
		skipped_upper_data,
	};
};

const is_active = (alert: JsonPerfAlert) =>
	alert?.status && alert.status === AlertStatus.Active;

// A boundary is skipped if it is defined but its limit undefined
// This indicates that the the boundary limit could not be calculated for the metric
const boundary_skipped = (
	boundary: undefined | Boundary,
	limit: undefined | number,
) => boundary && !limit;

const plot_marks = (
	plot_data,
	props: {
		project_slug: string;
		isConsole: boolean;
		plotId: string | undefined;
		lower_value: Accessor<boolean>;
		upper_value: Accessor<boolean>;
		lower_boundary: Accessor<boolean>;
		upper_boundary: Accessor<boolean>;
		x_axis_kind: string;
	},
) => {
	const plot_arrays = [];
	const warn_arrays = [];
	const alert_arrays = [];

	const colors = d3.schemeTableau10;

	for (const data of plot_data) {
		const {
			index,
			result,
			line_data,
			lower_alert_data,
			upper_alert_data,
			boundary_data,
			skipped_lower_data,
			skipped_upper_data,
		} = data;

		const color = colors[index % 10] ?? "7f7f7f";

		// Line
		plot_arrays.push(
			Plot.line(line_data, {
				x: props.x_axis_kind,
				y: "value",
				stroke: color,
			}),
		);
		// Dots
		plot_arrays.push(
			Plot.dot(line_data, {
				x: props.x_axis_kind,
				y: "value",
				symbol: "circle",
				stroke: color,
				fill: color,
				title: (datum) =>
					to_title(
						`${prettyPrintNumber(datum?.raw?.value)}`,
						result,
						datum,
						"\nClick to view Metric",
					),
				href: (datum) =>
					dotUrl(props.project_slug, props.isConsole, props.plotId, datum),
				target: "_top",
			}),
		);

		// Lower Value
		if (props.lower_value()) {
			plot_arrays.push(
				Plot.line(
					line_data,
					value_end_line(props.x_axis_kind, BoundaryLimit.Lower, color),
				),
			);
			plot_arrays.push(
				Plot.dot(
					line_data,
					value_end_dot(props.x_axis_kind, BoundaryLimit.Lower, color, result),
				),
			);
		}

		// Upper Value
		if (props.upper_value()) {
			plot_arrays.push(
				Plot.line(
					line_data,
					value_end_line(props.x_axis_kind, BoundaryLimit.Upper, color),
				),
			);
			plot_arrays.push(
				Plot.dot(
					line_data,
					value_end_dot(props.x_axis_kind, BoundaryLimit.Upper, color, result),
				),
			);
		}

		// Lower Boundary
		if (props.lower_boundary()) {
			plot_arrays.push(
				Plot.line(
					line_data,
					boundary_line(props.x_axis_kind, BoundaryLimit.Lower, color),
				),
			);
			plot_arrays.push(
				Plot.dot(
					boundary_data,
					boundary_dot(
						props.x_axis_kind,
						BoundaryLimit.Lower,
						color,
						props.project_slug,
						result,
						props.isConsole,
					),
				),
			);
			warn_arrays.push(
				Plot.image(
					skipped_lower_data,
					warning_image(
						props.x_axis_kind,
						props.project_slug,
						props.isConsole,
						props.plotId,
					),
				),
			);
		}

		// Upper Boundary
		if (props.upper_boundary()) {
			plot_arrays.push(
				Plot.line(
					line_data,
					boundary_line(props.x_axis_kind, BoundaryLimit.Upper, color),
				),
			);
			plot_arrays.push(
				Plot.dot(
					boundary_data,
					boundary_dot(
						props.x_axis_kind,
						BoundaryLimit.Upper,
						color,
						props.project_slug,
						result,
						props.isConsole,
					),
				),
			);
			warn_arrays.push(
				Plot.image(
					skipped_upper_data,
					warning_image(props.x_axis_kind, props.project_slug, props.isConsole),
				),
			);
		}

		alert_arrays.push(
			Plot.image(
				lower_alert_data,
				alert_image(
					props.x_axis_kind,
					BoundaryLimit.Lower,
					props.project_slug,
					result,
					props.isConsole,
					props.plotId,
				),
			),
		);
		alert_arrays.push(
			Plot.image(
				upper_alert_data,
				alert_image(
					props.x_axis_kind,
					BoundaryLimit.Upper,
					props.project_slug,
					result,
					props.isConsole,
					props.plotId,
				),
			),
		);
	}

	// This allows the alert images to appear on top of the plot lines.
	plot_arrays.push(...warn_arrays, ...alert_arrays);

	return plot_arrays;
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
	plotId: string | undefined,
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
		href: (datum) => thresholdUrl(project_slug, isConsole, plotId, datum),
		target: "_top",
	};
};

const warning_image = (
	x_axis: string,
	project_slug: string,
	isConsole: boolean,
	plotId: string | undefined,
) => {
	return {
		x: x_axis,
		y: "y",
		src: WARNING_URL,
		width: 18,
		title: (_datum) =>
			"Boundary Limit was not calculated.\nThis can happen for a couple of reasons:\n- There is not enough data yet (n < 2) (Most Common)\n- All the metric values are the same (variance == 0)\nClick to view Threshold",
		href: (datum) => thresholdUrl(project_slug, isConsole, plotId, datum),
		target: "_top",
	};
};

const dotUrl = (
	project_slug: string,
	isConsole: boolean,
	plotId: string | undefined,
	datum: object,
) =>
	`${resourcePath(isConsole)}/${project_slug}/metrics/${
		datum.metric
	}?${BACK_PARAM}=${encodePath(plotId)}`;

const thresholdUrl = (
	project_slug: string,
	isConsole: boolean,
	plotId: string | undefined,
	datum: object,
) =>
	`${resourcePath(isConsole)}/${project_slug}/thresholds/${
		datum.threshold?.uuid
	}?model=${datum.threshold?.model?.uuid}&${BACK_PARAM}=${encodePath(plotId)}`;

const alert_image = (
	x_axis: string,
	limit: BoundaryLimit,
	project_slug: string,
	result: object,
	isConsole: boolean,
	plotId: string | undefined,
) => {
	return {
		x: x_axis,
		y: boundary_position_key(limit),
		src: SIREN_URL,
		width: 18,
		title: (datum) =>
			limit_title(limit, result, datum, "\nClick to view Alert"),
		href: (datum) =>
			`${resourcePath(isConsole)}/${project_slug}/alerts/${
				datum.alert?.uuid
			}?${BACK_PARAM}=${encodePath(plotId)}`,
		target: "_top",
	};
};

const value_end_title = (limit: BoundaryLimit, result, datum, suffix) =>
	to_title(
		`${position_label(limit)} Value: ${prettyPrintNumber(datum?.raw?.[value_end_position_key(limit)])}`,
		result,
		datum,
		suffix,
	);

const limit_title = (limit: BoundaryLimit, result, datum, suffix) =>
	to_title(
		`${position_label(limit)} Limit: ${prettyPrintNumber(datum?.raw?.[boundary_position_key(limit)])}`,
		result,
		datum,
		suffix,
	);

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

function prettyPrintNumber(float: number | undefined) {
	return float?.toLocaleString("en-US", {
		minimumFractionDigits: 2,
		maximumFractionDigits: 2,
	});
}

export default LinePlot;
