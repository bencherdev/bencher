import * as Plot from "@observablehq/plot";
import * as d3 from "d3";
import {
	type Accessor,
	type Resource,
	Show,
	createEffect,
	createMemo,
	createSignal,
} from "solid-js";
import { createStore } from "solid-js/store";
import { resourcePath } from "../../../../../config/util";
import {
	AlertStatus,
	type Boundary,
	BoundaryLimit,
	type JsonMeasure,
	type JsonPerf,
	type JsonPerfAlert,
	type JsonPerfMetrics,
	XAxis,
} from "../../../../../types/bencher";
import { prettyPrintFloat } from "../../../../../util/convert";
import { scale_factor, scale_units } from "../../../../../util/scale";
import { BACK_PARAM, encodePath } from "../../../../../util/url";
import { Theme } from "../../../../navbar/theme/theme";
import { addTooltips } from "./tooltip";

// Source: https://twemoji.twitter.com
// License: https://creativecommons.org/licenses/by/4.0
const WARNING_URL =
	"https://s3.amazonaws.com/public.bencher.dev/perf/warning.png";
const SIREN_URL = "https://s3.amazonaws.com/public.bencher.dev/perf/siren.png";
const DEFAULT_UNITS = "units";

export interface Props {
	theme: Accessor<Theme>;
	isConsole: boolean;
	plotId: string | undefined;
	perfData: Resource<JsonPerf>;
	measures: Accessor<string[]>;
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
	const [y_left_label_area_size, set_left_y_label_area_size] =
		createSignal(512);
	const [y_right_label_area_size, set_right_y_label_area_size] =
		createSignal(0);
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
	const [perfActive, setPerfActive] = createStore(props.perfActive);

	const SCALE_FACTOR = 1.12;
	createEffect(() => {
		if (isPlotted()) {
			const tagIdEscaped = CSS.escape(tagId());
			const y_axes = document.querySelectorAll(
				`#${tagIdEscaped} svg [aria-label='y-axis tick label']`,
			);
			if (!y_axes) {
				return;
			}
			const [left_axis, right_axis] = y_axes;
			if (left_axis !== undefined) {
				const width = left_axis.getBoundingClientRect().width;
				set_left_y_label_area_size(width * SCALE_FACTOR);
			}
			if (right_axis !== undefined) {
				const width = right_axis.getBoundingClientRect().width;
				set_right_y_label_area_size(width * SCALE_FACTOR);
			}
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
		} else if (!arraysEqual(props.perfActive, perfActive)) {
			setPerfActive(props.perfActive);
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
								type: linePlot()?.x_axis?.scale_type,
								grid: true,
								label: `${linePlot()?.x_axis?.label}`,
								labelOffset: 36,
							},
							y: {
								grid: true,
							},
							marks: linePlot().marks,
							width: props.width(),
							nice: true,
							// https://github.com/observablehq/plot/blob/main/README.md#layout-options
							// For simplicity’s sake and for consistent layout across plots, margins are not automatically sized to make room for tick labels; instead, shorten your tick labels or increase the margins as needed.
							marginLeft: y_left_label_area_size(),
							marginRight: y_right_label_area_size(),
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

const arraysEqual = (a: boolean[], b: boolean[]): boolean => {
	if (a.length !== b.length) return false;
	for (let i = 0; i < a.length; i++) {
		if (a[i] !== b[i]) return false;
	}
	return true;
};

const line_plot = (props: Props) => {
	const json_perf = props.perfData();
	// console.log(json_perf);

	const NOT_FOUND = {
		metrics_found: false,
	};
	if (
		typeof json_perf !== "object" ||
		json_perf === undefined ||
		json_perf === null ||
		!Array.isArray(json_perf.results)
	) {
		return NOT_FOUND;
	}

	const [left_measure, right_measure] = get_measures(json_perf, props.measures);
	if (!left_measure && !right_measure) {
		return NOT_FOUND;
	}

	const active_data = json_perf.results
		.map(perf_result)
		.filter((datum) => props.perfActive[datum.index]);
	const hasData = (measure: undefined | JsonMeasure) =>
		active_data.reduce(
			(acc, data) =>
				acc ||
				(data?.result?.measure?.uuid === measure?.uuid &&
					(data?.line_data?.length ?? 0) > 0),
			false,
		);
	const left_has_data = hasData(left_measure);
	const right_has_data = hasData(right_measure);
	const metrics_found = left_has_data || right_has_data;
	if (!metrics_found) {
		return NOT_FOUND;
	}

	const scale_props = {
		lower_value: props.lower_value,
		upper_value: props.upper_value,
		lower_boundary: props.lower_boundary,
		upper_boundary: props.upper_boundary,
	};

	const [scaled_data, scales] = scale_data(
		active_data,
		left_measure,
		left_has_data,
		right_measure,
		right_has_data,
		scale_props,
	);

	const x_axis = get_x_axis(props.x_axis());

	const axis_marks = [];

	if (left_has_data) {
		const y_axis = get_y_axis(scales, left_measure, Anchor.Left);
		if (y_axis) {
			axis_marks.push(y_axis);
		}
	}
	if (right_has_data) {
		const y_axis = get_y_axis(scales, right_measure, Anchor.Right);
		if (y_axis) {
			axis_marks.push(y_axis);
		}
	}

	const marks = plot_marks(scaled_data, scales, axis_marks, {
		project_slug: json_perf.project.slug,
		isConsole: props.isConsole,
		plotId: props.plotId,
		x_axis_kind: x_axis.kind,
		...scale_props,
	});

	return {
		metrics_found,
		x_axis,
		marks,
		hoverStyles: hover_styles(props.theme()),
	};
};

const get_x_axis = (x_axis: XAxis) => {
	switch (x_axis) {
		case XAxis.DateTime:
			return {
				kind: "date_time",
				scale_type: "time",
				label: "Report Date and Time",
			};
		case XAxis.Version:
			return {
				kind: "number",
				scale_type: "point",
				label: "Branch Version Number →",
			};
	}
};

const get_measures = (json_perf: JsonPerf, measures: Accessor<string[]>) => {
	const findMeasure = (uuid: undefined | string) =>
		json_perf?.results?.find((result) => result?.measure?.uuid === uuid)
			?.measure;
	const [first_measure_uuid, second_measure_uuid] = measures();
	return [findMeasure(first_measure_uuid), findMeasure(second_measure_uuid)];
};

enum Anchor {
	Left = "left",
	Right = "right",
}
const get_y_axis = (
	scales: undefined | Scales,
	measure: undefined | JsonMeasure,
	anchor: Anchor,
) => {
	if (!measure?.uuid) {
		return;
	}
	const scale = scales?.[measure?.uuid];
	const yScale = scale?.yScale;
	if (yScale) {
		return Plot.axisY(yScale?.ticks(), {
			anchor,
			label: `↑ ${scale?.units}`,
			y: yScale,
			tickFormat: yScale?.tickFormat(),
		});
	}
	return;
};

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

const perf_result = (result: JsonPerfMetrics, index: number) => {
	const line_data = [];
	const lower_alert_data = [];
	const upper_alert_data = [];
	const boundary_data = [];
	const skipped_lower_data = [];
	const skipped_upper_data = [];

	for (const perf_metric of result.metrics) {
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
				iteration: datum.iteration,
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
				iteration: datum.iteration,
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

type Scales = {
	[uuid: string]: Scale;
};
type Scale = {
	measure: JsonMeasure;
	factor: number;
	units: string;
	yScale: d3.ScalePower<number, number, never>;
};

const scale_data = (
	data: object[],
	left_measure: undefined | JsonMeasure,
	left_has_data: boolean,
	right_measure: undefined | JsonMeasure,
	right_has_data: boolean,
	props: {
		lower_value: Accessor<boolean>;
		upper_value: Accessor<boolean>;
		lower_boundary: Accessor<boolean>;
		upper_boundary: Accessor<boolean>;
	},
): [object[], Scales?] => {
	const left_scale = get_scale(data, left_measure, left_has_data, props);
	const right_scale = get_scale(data, right_measure, right_has_data, props);
	const scaled_data = scale_data_by_factor(data, left_scale, right_scale);
	return [
		scaled_data,
		{
			...(left_measure && left_scale && { [left_measure?.uuid]: left_scale }),
			...(right_measure &&
				right_scale && { [right_measure?.uuid]: right_scale }),
		},
	];
};

const get_scale = (
	data: object[],
	measure: undefined | JsonMeasure,
	has_data: boolean,
	props: {
		lower_value: Accessor<boolean>;
		upper_value: Accessor<boolean>;
		lower_boundary: Accessor<boolean>;
		upper_boundary: Accessor<boolean>;
	},
): undefined | Scale => {
	if (!measure || !has_data) {
		return;
	}

	const min = data_min(data, measure, props);
	const max = data_max(data, measure, props);

	const raw_units = measure?.units ?? DEFAULT_UNITS;
	const factor = scale_factor(min, raw_units);
	const units = scale_units(min, raw_units);

	const domain = [min / factor, max / factor];
	// Use pow scaling to allow users to more easily reason on graphs with highly differentiated values
	// See: https://observablehq.com/plot/features/scales#continuous-scales
	const yScale = d3.scalePow().exponent(0.5).domain(domain).nice();

	return {
		measure,
		factor,
		units,
		yScale,
	};
};

const MAX = Number.MAX_SAFE_INTEGER;
const data_min = (
	raw_data: object[],
	measure: JsonMeasure,
	props: {
		lower_value: Accessor<boolean>;
		upper_value: Accessor<boolean>;
		lower_boundary: Accessor<boolean>;
		upper_boundary: Accessor<boolean>;
	},
) =>
	Math.min(
		...raw_data
			.filter((data) => data?.result?.measure?.uuid === measure?.uuid)
			.map((data) =>
				Math.min(
					// The primary metric series
					Math.min(
						...(data.line_data?.map((datum) => datum.value ?? MAX) ?? MAX),
					),
					// The lower value series, if active
					props.lower_value()
						? Math.min(
								...(data.line_data?.map((datum) => datum.lower_value ?? MAX) ??
									MAX),
							)
						: MAX,
					// The upper value series, if active
					props.upper_value()
						? Math.min(
								...(data.line_data?.map((datum) => datum.upper_value ?? MAX) ??
									MAX),
							)
						: MAX,
					// The lower boundary series, if active
					props.lower_boundary()
						? Math.min(
								...(data.line_data?.map((datum) => datum.lower_limit ?? MAX) ??
									MAX),
								...(data.skipped_lower_data?.map((datum) => datum.y ?? MAX) ??
									MAX),
							)
						: MAX,
					// The upper boundary series, if active
					props.upper_boundary()
						? Math.min(
								...(data.line_data?.map((datum) => datum.upper_limit ?? MAX) ??
									MAX),
								...(data.skipped_upper_data?.map((datum) => datum.y ?? MAX) ??
									MAX),
							)
						: MAX,
					// The lower alert series
					Math.min(
						...(data.lower_alert_data?.map(
							(datum) => datum.lower_limit ?? MAX,
						) ?? MAX),
					),
					// The upper alert series
					Math.min(
						...(data.upper_alert_data?.map(
							(datum) => datum.upper_limit ?? MAX,
						) ?? MAX),
					),
				),
			),
	);

const MIN = Number.MIN_SAFE_INTEGER;
const data_max = (
	raw_data: object[],
	measure: JsonMeasure,
	props: {
		lower_value: Accessor<boolean>;
		upper_value: Accessor<boolean>;
		lower_boundary: Accessor<boolean>;
		upper_boundary: Accessor<boolean>;
	},
) =>
	Math.max(
		...raw_data
			.filter((data) => data?.result?.measure?.uuid === measure?.uuid)
			.map((data) =>
				Math.max(
					// The primary metric series
					Math.max(
						...(data.line_data?.map((datum) => datum.value ?? MIN) ?? MIN),
					),
					// The lower value series, if active
					props.lower_value()
						? Math.max(
								...(data.line_data?.map((datum) => datum.lower_value ?? MIN) ??
									MIN),
							)
						: MIN,
					// The upper value series, if active
					props.upper_value()
						? Math.max(
								...(data.line_data?.map((datum) => datum.upper_value ?? MIN) ??
									MIN),
							)
						: MIN,
					// The lower boundary series, if active
					props.lower_boundary()
						? Math.max(
								...(data.line_data?.map((datum) => datum.lower_limit ?? MIN) ??
									MIN),
								...(data.skipped_lower_data?.map((datum) => datum.y ?? MIN) ??
									MIN),
							)
						: MIN,
					// The upper boundary series, if active
					props.upper_boundary()
						? Math.max(
								...(data.line_data?.map((datum) => datum.upper_limit ?? MIN) ??
									MIN),
								...(data.skipped_upper_data?.map((datum) => datum.y ?? MIN) ??
									MIN),
							)
						: MIN,
					// The lower alert series
					Math.max(
						...(data.lower_alert_data?.map(
							(datum) => datum.lower_limit ?? MIN,
						) ?? MIN),
					),
					// The upper alert series
					Math.max(
						...(data.upper_alert_data?.map(
							(datum) => datum.upper_limit ?? MIN,
						) ?? MIN),
					),
				),
			),
	);

type AxisScale = {
	measure: JsonMeasure;
	factor: number;
};

const scale_data_by_factor = (
	input_data: object[],
	left: undefined | AxisScale,
	right: undefined | AxisScale,
) => {
	// We need to get the scale factor for each datum individually since data from both measures is intermixed.
	const scale_factor = (data) => {
		if (data?.result?.measure?.uuid === left?.measure?.uuid) {
			return left?.factor;
		}
		if (data?.result?.measure?.uuid === right?.measure?.uuid) {
			return right?.factor;
		}
		return;
	};

	const map_limits = (datum, factor: number) => {
		if (datum.lower_limit !== undefined && datum.lower_limit !== null) {
			datum.lower_limit = datum.lower_limit / factor;
		}
		if (datum.upper_limit !== undefined && datum.upper_limit !== null) {
			datum.upper_limit = datum.upper_limit / factor;
		}
		return datum;
	};

	return input_data.map((data) => {
		const factor = scale_factor(data);
		if (!factor) {
			return data;
		}

		data.line_data = data.line_data?.map((datum) => {
			datum.value = datum.value / factor;
			if (datum.lower_value !== undefined && datum.lower_value !== null) {
				datum.lower_value = datum.lower_value / factor;
			}
			if (datum.upper_value !== undefined && datum.upper_value !== null) {
				datum.upper_value = datum.upper_value / factor;
			}
			return map_limits(datum, factor);
		});
		data.lower_alert_data = data.lower_alert_data?.map((datum) =>
			map_limits(datum, factor),
		);
		data.upper_alert_data = data.upper_alert_data?.map((datum) =>
			map_limits(datum, factor),
		);
		data.boundary_data = data.boundary_data?.map((datum) =>
			map_limits(datum, factor),
		);
		data.skipped_lower_data = data.skipped_lower_data?.map((datum) => {
			datum.y = datum.y / factor;
			return datum;
		});
		data.skipped_upper_data = data.skipped_upper_data?.map((datum) => {
			datum.y = datum.y / factor;
			return datum;
		});

		return data;
	});
};

type Series = (Plot.Line | Plot.Dot | Plot.Image | Plot.CompoundMark)[];

const plot_marks = (
	plot_data,
	plot_scales: undefined | Scales,
	axis_marks: Plot.CompoundMark[],
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
): Series => {
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
		const scale = plot_scales?.[result?.measure?.uuid];
		if (!scale) {
			continue;
		}
		const units = scale?.units ?? DEFAULT_UNITS;

		const color = colors[index % 10] ?? "7f7f7f";

		// Line
		const line_options = {
			x: props.x_axis_kind,
			y: "value",
			stroke: color,
		};
		plot_arrays.push(Plot.lineY(line_data, map_options(scale, line_options)));
		// Dots
		const dot_options = {
			x: props.x_axis_kind,
			y: "value",
			symbol: "circle",
			stroke: color,
			fill: color,
			title: (datum) =>
				to_title(
					`${prettyPrintFloat(datum?.value)}\n${units}`,
					result,
					datum,
					"\nClick to view Metric",
				),
			href: (datum) =>
				dotUrl(props.project_slug, props.isConsole, props.plotId, datum),
			target: "_top",
		};
		plot_arrays.push(Plot.dotY(line_data, map_options(scale, dot_options)));

		// Lower Value
		if (props.lower_value()) {
			plot_arrays.push(
				Plot.lineY(
					line_data,
					value_end_line(scale, props.x_axis_kind, BoundaryLimit.Lower, color),
				),
			);
			plot_arrays.push(
				Plot.dotY(
					line_data,
					value_end_dot(
						scale,
						props.x_axis_kind,
						BoundaryLimit.Lower,
						color,
						result,
						units,
					),
				),
			);
		}

		// Upper Value
		if (props.upper_value()) {
			plot_arrays.push(
				Plot.lineY(
					line_data,
					value_end_line(scale, props.x_axis_kind, BoundaryLimit.Upper, color),
				),
			);
			plot_arrays.push(
				Plot.dotY(
					line_data,
					value_end_dot(
						scale,
						props.x_axis_kind,
						BoundaryLimit.Upper,
						color,
						result,
						units,
					),
				),
			);
		}

		// Lower Boundary
		if (props.lower_boundary()) {
			plot_arrays.push(
				Plot.line(
					line_data,
					boundary_line(scale, props.x_axis_kind, BoundaryLimit.Lower, color),
				),
			);
			plot_arrays.push(
				Plot.dot(
					boundary_data,
					boundary_dot(
						scale,
						props.x_axis_kind,
						BoundaryLimit.Lower,
						color,
						props.project_slug,
						result,
						units,
						props.isConsole,
					),
				),
			);
			warn_arrays.push(
				Plot.image(
					skipped_lower_data,
					warning_image(
						scale,
						props.x_axis_kind,
						BoundaryLimit.Lower,
						props.project_slug,
						result,
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
					boundary_line(scale, props.x_axis_kind, BoundaryLimit.Upper, color),
				),
			);
			plot_arrays.push(
				Plot.dot(
					boundary_data,
					boundary_dot(
						scale,
						props.x_axis_kind,
						BoundaryLimit.Upper,
						color,
						props.project_slug,
						result,
						units,
						props.isConsole,
					),
				),
			);
			warn_arrays.push(
				Plot.image(
					skipped_upper_data,
					warning_image(
						scale,
						props.x_axis_kind,
						BoundaryLimit.Upper,
						props.project_slug,
						result,
						props.isConsole,
					),
				),
			);
		}

		alert_arrays.push(
			Plot.image(
				lower_alert_data,
				alert_image(
					scale,
					props.x_axis_kind,
					BoundaryLimit.Lower,
					props.project_slug,
					result,
					units,
					props.isConsole,
					props.plotId,
				),
			),
		);
		alert_arrays.push(
			Plot.image(
				upper_alert_data,
				alert_image(
					scale,
					props.x_axis_kind,
					BoundaryLimit.Upper,
					props.project_slug,
					result,
					units,
					props.isConsole,
					props.plotId,
				),
			),
		);
	}

	// This allows the alert images to appear on top of the plot lines,
	// and the plot lines to appear on top of the axis marks.
	(axis_marks as Series).push(...plot_arrays, ...warn_arrays, ...alert_arrays);

	return axis_marks;
};

// https://observablehq.com/@observablehq/plot-dual-axis
const map_options = (scale: Scale, options: object) =>
	Plot.mapY((D) => D.map(scale?.yScale), options);

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
	scale: Scale,
	x_axis: string,
	limit: BoundaryLimit,
	color: string,
) => {
	return map_options(scale, {
		x: x_axis,
		y: value_end_position_key(limit),
		stroke: color,
		strokeWidth: 2,
		strokeOpacity: 0.9,
		strokeDasharray: [3],
	});
};

const value_end_dot = (
	scale: Scale,
	x_axis: string,
	limit: BoundaryLimit,
	color: string,
	result: object,
	units: string,
) => {
	return map_options(scale, {
		x: x_axis,
		y: value_end_position_key(limit),
		symbol: "diamond",
		stroke: color,
		strokeWidth: 2,
		strokeOpacity: 0.9,
		fill: color,
		fillOpacity: 0.9,
		title: (datum) => value_end_title(limit, result, datum, units),
	});
};

const boundary_line = (
	scale: Scale,
	x_axis: string,
	limit: BoundaryLimit,
	color,
) => {
	return map_options(scale, {
		x: x_axis,
		y: boundary_position_key(limit),
		stroke: color,
		strokeWidth: 4,
		strokeOpacity: 0.666,
		strokeDasharray: [8],
	});
};

const boundary_dot = (
	scale: Scale,
	x_axis: string,
	limit: BoundaryLimit,
	color: string,
	project_slug: string,
	result: object,
	units: string,
	isConsole: boolean,
	plotId: string | undefined,
) => {
	return map_options(scale, {
		x: x_axis,
		y: boundary_position_key(limit),
		symbol: "square",
		stroke: color,
		strokeWidth: 4,
		strokeOpacity: 0.666,
		fill: color,
		fillOpacity: 0.666,
		title: (datum) =>
			limit_title(limit, result, datum, units, "\nClick to view Threshold"),
		href: (datum) => thresholdUrl(project_slug, isConsole, plotId, datum),
		target: "_top",
	});
};

const warning_image = (
	scale: Scale,
	x_axis: string,
	limit: BoundaryLimit,
	project_slug: string,
	result,
	isConsole: boolean,
	plotId: string | undefined,
) => {
	return map_options(scale, {
		x: x_axis,
		y: "y",
		src: WARNING_URL,
		width: 18,
		title: (datum) =>
			to_title(
				`${position_label(limit)} Boundary Limit was not calculated\nThis can happen for a couple of reasons:\n- There is not enough data yet (n < 2) (Most Common)\n- All the metric values are the same (variance == 0)`,
				result,
				datum,
				"\nClick to view Threshold",
			),
		href: (datum) => thresholdUrl(project_slug, isConsole, plotId, datum),
		target: "_top",
	});
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
	scale: Scale,
	x_axis: string,
	limit: BoundaryLimit,
	project_slug: string,
	result: object,
	units: string,
	isConsole: boolean,
	plotId: string | undefined,
) => {
	return map_options(scale, {
		x: x_axis,
		y: boundary_position_key(limit),
		src: SIREN_URL,
		width: 18,
		title: (datum) =>
			limit_title(limit, result, datum, units, "\nClick to view Alert"),
		href: (datum) =>
			`${resourcePath(isConsole)}/${project_slug}/alerts/${
				datum.alert?.uuid
			}?${BACK_PARAM}=${encodePath(plotId)}`,
		target: "_top",
	});
};

const value_end_title = (limit: BoundaryLimit, result, datum, units) =>
	to_title(
		`${position_label(limit)} Value\n${prettyPrintFloat(datum?.[value_end_position_key(limit)])}\n${units}`,
		result,
		datum,
		"",
	);

const limit_title = (limit: BoundaryLimit, result, datum, units, suffix) =>
	to_title(
		`${position_label(limit)} Boundary Limit\n${prettyPrintFloat(datum?.[boundary_position_key(limit)])}\n${units}`,
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

export default LinePlot;
