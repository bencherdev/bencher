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

const arraysEqual = (a: any[], b: any[]): boolean => {
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
	if (!left_measure) {
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
	if (!left_has_data && !right_has_data) {
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
		right_measure,
		scale_props,
	);

	const x_axis = get_x_axis(props.x_axis());

	const marks = plot_marks(scaled_data, scales, {
		project_slug: json_perf.project.slug,
		isConsole: props.isConsole,
		plotId: props.plotId,
		x_axis_kind: x_axis.kind,
		perfActive: props.perfActive,
		...scale_props,
	});

	if (left_has_data) {
		const left_scale = scales?.[left_measure?.uuid];
		const yScale = left_scale?.yScale;
		if (yScale) {
			console.log("LEFT TICKS", yScale?.ticks());
			marks.push(
				Plot.axisY(yScale?.ticks(), {
					anchor: "left",
					label: `↑ ${left_scale?.units}`,
					y: yScale,
					tickFormat: yScale?.tickFormat(),
				}),
			);
		}
	}

	// The `raw_data_clone` is only created if there is a second measure
	if (right_has_data) {
		const right_scale = scales?.[right_measure?.uuid];
		const yScale = right_scale?.yScale;
		if (yScale) {
			console.log("RIGHT TICKS", yScale?.ticks());
			marks.push(
				Plot.axisY(yScale?.ticks(), {
					anchor: "right",
					label: `↑ ${right_scale?.units}`,
					y: yScale,
					tickFormat: yScale?.tickFormat(),
				}),
			);
		}
	}

	return {
		metrics_found: left_has_data || right_has_data,
		x_axis,
		y_axis: scales?.[left_measure?.uuid],
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

// const active_data = (plot_data, perfActive) =>
// 	plot_data.filter((datum) => perfActive[datum.index]);

const get_measures = (json_perf: JsonPerf, measures: Accessor<string[]>) => {
	const findMeasure = (uuid: undefined | string) =>
		json_perf?.results?.find((result) => result?.measure?.uuid === uuid)
			?.measure;
	const [first_measure_uuid, second_measure_uuid] = measures();
	return [findMeasure(first_measure_uuid), findMeasure(second_measure_uuid)];
};

const tickFormat = prettyPrintFloat;

const clone_raw_data = (raw_data, second_measure) => {
	if (second_measure) {
		return JSON.parse(JSON.stringify(raw_data));
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
			// Display values: scaled but not relative when multi-axis
			display: {
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
			// Display values: scaled but not relative when multi-axis
			display: {
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
	[uuid: string]: {
		measure: JsonMeasure;
		factor: number;
		units: string;
		yScale?: d3.ScaleLinear<number, number, never>;
	};
};

const scale_data = (
	raw_data: object[],
	left_measure: JsonMeasure,
	right_measure: undefined | JsonMeasure,
	props: {
		lower_value: Accessor<boolean>;
		upper_value: Accessor<boolean>;
		lower_boundary: Accessor<boolean>;
		upper_boundary: Accessor<boolean>;
	},
): [object[], Scales?] => {
	const raw_left_units = left_measure?.units ?? DEFAULT_UNITS;

	const left_min = data_min(raw_data, left_measure, props);
	const left_max = data_max(raw_data, left_measure, props);
	const left_factor = scale_factor(left_min, raw_left_units);
	const left_scaled_units = scale_units(left_min, raw_left_units);
	const left_has_data = left_min < MAX;

	const left_domain = [left_min / left_factor, left_max / left_factor];
	console.log("LEFT DOMAIN", left_domain);
	const leftScale = d3.scaleLinear().domain(left_domain).nice();

	const left_scale = {
		measure: left_measure,
		factor: left_factor,
		units: left_scaled_units,
		yScale: leftScale,
	};
	if (!right_measure) {
		if (left_has_data) {
			const scaled_data = scale_data_by_factor(raw_data, left_scale);
			return [
				scaled_data,
				{
					[left_measure?.uuid]: left_scale,
				},
			];
		}
		return [raw_data];
	}

	const raw_right_units = right_measure?.units ?? DEFAULT_UNITS;

	const right_min = data_min(raw_data, right_measure, props);
	const right_max = data_max(raw_data, right_measure, props);
	const right_factor = scale_factor(right_min, raw_right_units);
	const right_scaled_units = scale_units(right_min, raw_right_units);

	// const min_ratio = left_min / right_min;
	// console.log("MIN RATIO", min_ratio);
	// const max_ratio = left_max / right_max;
	// console.log("MAX RATIO", max_ratio);
	// const ratio = 1 / Math.max(min_ratio, max_ratio);
	// console.log("RATIO", ratio);
	const ratio = 1;

	const right_domain = [right_min / right_factor, right_max / right_factor];
	console.log("RIGHT DOMAIN", right_domain);
	const rightScale = d3.scaleLinear().domain(right_domain).nice();

	const right_scale = {
		measure: right_measure,
		factor: right_factor,
		units: right_scaled_units,
		ratio,
		yScale: rightScale,
	};
	const scaled_data = scale_data_by_factor(raw_data, left_scale, right_scale);
	return [
		scaled_data,
		{
			[left_measure?.uuid]: left_scale,
			[right_measure?.uuid]: right_scale,
		},
	];
};

const right_y_axis_ticks = (
	raw_data: object[],
	first_measure: JsonMeasure,
	second_measure: JsonMeasure,
	props: {
		lower_value: Accessor<boolean>;
		upper_value: Accessor<boolean>;
		lower_boundary: Accessor<boolean>;
		upper_boundary: Accessor<boolean>;
	},
) => {
	const raw_second_units = second_measure?.units ?? DEFAULT_UNITS;

	const second_min = data_min(raw_data, second_measure, props);
	const second_factor = scale_factor(second_min, raw_second_units);

	const second = {
		measure: second_measure,
		factor: second_factor,
	};
	const second_measure_data = raw_data.filter(
		(datum) => datum?.result?.measure?.uuid === second_measure?.uuid,
	);
	const scaled_data = scale_data_by_factor(second_measure_data, second);

	const axis_data = [];
	const axis_limits = (datum) => {
		if (
			props.lower_boundary() &&
			datum.lower_boundary !== undefined &&
			datum.lower_boundary !== null
		) {
			axis_data.push(datum.lower_boundary);
		}
		if (
			props.upper_boundary() &&
			datum.upper_boundary !== undefined &&
			datum.upper_boundary !== null
		) {
			axis_data.push(datum.upper_boundary);
		}
	};
	for (const data of scaled_data) {
		for (const datum of data?.line_data) {
			axis_data.push(datum.value);
			if (
				props.lower_value() &&
				datum.lower_value !== undefined &&
				datum.lower_value !== null
			) {
				axis_data.push(datum.lower_value);
			}
			if (
				props.upper_value() &&
				datum.upper_value !== undefined &&
				datum.upper_value !== null
			) {
				axis_data.push(datum.upper_value);
			}
			axis_limits(datum);
		}
		for (const datum in data?.lower_alert_data) {
			axis_limits(datum);
		}
		for (const datum in data?.upper_alert_data) {
			axis_limits(datum);
		}
		for (const datum in data?.boundary_data) {
			axis_limits(datum);
		}
		for (const datum in data?.skipped_lower_data) {
			axis_data.push(datum.y);
		}
		for (const datum in data?.skipped_upper_data) {
			axis_data.push(datum.y);
		}
	}

	// Calculate the range of axis_data
	const axis_min = Math.min(...axis_data);
	const axis_max = Math.max(...axis_data);

	const first_min = data_min(raw_data, first_measure, props);
	const first_max = data_max(raw_data, first_measure, props);

	// Create a linear scale based on the range of axis_data
	// return d3.scaleLinear([axis_min, axis_max], [first_min, first_max]);
	return d3.scaleLinear([axis_min, axis_max]);
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

const scale_data_by_factor = (
	raw_data: object[],
	first: {
		measure: JsonMeasure;
		factor: number;
	},
	second?: {
		measure: JsonMeasure;
		factor: number;
		ratio: number;
	},
) => {
	const scales = (data) => {
		if (data?.result?.measure?.uuid === first?.measure?.uuid) {
			return [first?.factor];
		}
		if (data?.result?.measure?.uuid === second?.measure?.uuid) {
			return [second?.factor, second?.ratio];
		}
		return [];
	};

	const map_limits = (datum, factor: number, ratio: number) => {
		if (datum.lower_limit !== undefined && datum.lower_limit !== null) {
			datum.lower_limit = datum.lower_limit / factor;
			datum.display.lower_limit = datum.lower_limit;
			if (ratio) {
				datum.lower_limit = datum.lower_limit * ratio;
			}
		}
		if (datum.upper_limit !== undefined && datum.upper_limit !== null) {
			datum.upper_limit = datum.upper_limit / factor;
			datum.display.upper_limit = datum.upper_limit;
			if (ratio) {
				datum.upper_limit = datum.upper_limit * ratio;
			}
		}
		return datum;
	};

	return raw_data.map((data) => {
		const [factor, ratio] = scales(data);
		if (!factor) {
			return data;
		}

		data.line_data = data.line_data?.map((datum) => {
			datum.value = datum.value / factor;
			datum.display.value = datum.value;
			if (ratio) {
				datum.value = datum.value * ratio;
			}
			if (datum.lower_value !== undefined && datum.lower_value !== null) {
				datum.lower_value = datum.lower_value / factor;
				datum.display.lower_value = datum.lower_value;
				if (ratio) {
					datum.lower_value = datum.lower_value * ratio;
				}
			}
			if (datum.upper_value !== undefined && datum.upper_value !== null) {
				datum.upper_value = datum.upper_value / factor;
				datum.display.upper_value = datum.upper_value;
				if (ratio) {
					datum.upper_value = datum.upper_value * ratio;
				}
			}
			return map_limits(datum, factor, ratio);
		});
		data.lower_alert_data = data.lower_alert_data?.map((datum) =>
			map_limits(datum, factor, ratio),
		);
		data.upper_alert_data = data.upper_alert_data?.map((datum) =>
			map_limits(datum, factor, ratio),
		);
		data.boundary_data = data.boundary_data?.map((datum) =>
			map_limits(datum, factor, ratio),
		);
		data.skipped_lower_data = data.skipped_lower_data?.map((datum) => {
			datum.y = datum.y / factor;
			if (ratio) {
				datum.y = datum.y * ratio;
			}
			return datum;
		});
		data.skipped_upper_data = data.skipped_upper_data?.map((datum) => {
			datum.y = datum.y / factor;
			if (ratio) {
				datum.y = datum.y * ratio;
			}
			return datum;
		});

		return data;
	});
};

const plot_marks = (
	plot_data,
	plot_scales: Scales,
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
		const scale = plot_scales?.[result?.measure?.uuid];
		if (!scale) {
			continue;
		}
		const units = scale?.units ?? DEFAULT_UNITS;

		const color = colors[index % 10] ?? "7f7f7f";

		// Line
		const line_options = {
			x: props.x_axis_kind,
			y: (d) => {
				console.log("Y", d.value);
				return d.value;
			},
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
					`${prettyPrintFloat(datum?.display?.value)}\n${units}`,
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
						units,
						props.isConsole,
					),
				),
			);
			warn_arrays.push(
				Plot.image(
					skipped_lower_data,
					warning_image(
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
						units,
						props.isConsole,
					),
				),
			);
			warn_arrays.push(
				Plot.image(
					skipped_upper_data,
					warning_image(
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

	// This allows the alert images to appear on top of the plot lines.
	plot_arrays.push(...warn_arrays, ...alert_arrays);

	return plot_arrays;
};

const map_options = (scale, options: object) =>
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
	scale,
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
	scale,
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
	units: string,
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
			limit_title(limit, result, datum, units, "\nClick to view Threshold"),
		href: (datum) => thresholdUrl(project_slug, isConsole, plotId, datum),
		target: "_top",
	};
};

const warning_image = (
	x_axis: string,
	limit: BoundaryLimit,
	project_slug: string,
	result,
	isConsole: boolean,
	plotId: string | undefined,
) => {
	return {
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
	units: string,
	isConsole: boolean,
	plotId: string | undefined,
) => {
	return {
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
	};
};

const value_end_title = (limit: BoundaryLimit, result, datum, units) =>
	to_title(
		`${position_label(limit)} Value\n${prettyPrintFloat(datum?.display?.[value_end_position_key(limit)])}\n${units}`,
		result,
		datum,
		"",
	);

const limit_title = (limit: BoundaryLimit, result, datum, units, suffix) =>
	to_title(
		`${position_label(limit)} Boundary Limit\n${prettyPrintFloat(datum?.display?.[boundary_position_key(limit)])}\n${units}`,
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
