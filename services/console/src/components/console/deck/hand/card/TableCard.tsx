import type { Params } from "astro";
import {
	BoundaryLimit,
	type JsonAlert,
	type JsonReport,
	type JsonThreshold,
} from "../../../../../types/bencher";
import { createMemo, For, Show, type Resource } from "solid-js";
import { resourcePath } from "../../../../../config/util";
import { BACK_PARAM, encodePath } from "../../../../../util/url";
import { format } from "d3";
import { dateTimeMillis } from "../../../../../util/convert";

export interface Props {
	isConsole?: boolean;
	apiUrl: string;
	params: Params;
	value: Resource<JsonReport>;
}

const TableCard = (props: Props) => {
	const multipleIterations = createMemo(
		() => (props.value()?.results?.length ?? 0) > 1,
	);
	return (
		<div class="columns is-centered" style="margin-top: 2rem">
			<div class="column is-11">
				<Show when={(props.value()?.alerts?.length ?? 0) > 0}>
					<h3 class="title is-3">
						🚨 {props.value()?.alerts?.length} Alert
						{props.value()?.alerts?.length === 1 ? "" : "s"}
					</h3>
					<div class="table-container">
						<table class="table is-bordered is-fullwidth">
							<thead>
								<tr>
									{multipleIterations() && <th>Iteration</th>}
									<th>Benchmark</th>
									<th>Measure (units)</th>
									<th>View</th>
									<th>Value</th>
									<th>Lower Boundary</th>
									<th>Upper Boundary</th>
								</tr>
							</thead>
							<tbody>
								<For each={props.value()?.alerts}>
									{(alert) => {
										const value = alert?.metric?.value ?? 0;
										const baseline = alert?.boundary?.baseline ?? 0;
										const valuePercent =
											value > 0 && baseline > 0
												? ((value - baseline) / baseline) * 100
												: 0.0;
										const lowerLimit = alert?.boundary?.lower_limit;
										const upperLimit = alert?.boundary?.upper_limit;
										const lowerLimitPercentage =
											lowerLimit === undefined
												? 0
												: value > 0 && lowerLimit > 0
													? (lowerLimit / value) * 100
													: 0.0;
										const upperLimitPercentage =
											upperLimit === undefined
												? 0
												: value > 0 && upperLimit > 0
													? (value / upperLimit) * 100
													: 0.0;

										return (
											<tr>
												{multipleIterations() && <td>{alert?.iteration}</td>}
												<td>
													<a
														href={`${resourcePath(props.isConsole)}/${
															props.params?.project
														}/benchmarks/${
															alert?.benchmark?.slug
														}?${BACK_PARAM}=${encodePath()}`}
													>
														{alert?.benchmark?.name}
													</a>
												</td>
												<td>
													<a
														href={`${resourcePath(props.isConsole)}/${
															props.params?.project
														}/measures/${
															alert?.threshold?.measure?.slug
														}?${BACK_PARAM}=${encodePath()}`}
													>
														{alert?.threshold?.measure?.name}
													</a>
												</td>
												<td>
													📈{" "}
													<a
														href={alertPerfUrl(
															props.isConsole,
															props.params?.project,
															alert,
														)}
													>
														plot
													</a>
													<br />🚨{" "}
													<a
														href={alertUrl(
															props.isConsole,
															props.params?.project,
															alert,
														)}
													>
														alert
													</a>
													<br />🚷{" "}
													<a
														href={thresholdUrl(
															props.isConsole,
															props.params?.project,
															alert?.threshold,
														)}
													>
														threshold
													</a>
												</td>
												<td>
													<b>
														{`${formatNumber(value)} (${
															valuePercent > 0.0 ? "+" : ""
														}${formatNumber(valuePercent)}%)`}
													</b>
												</td>
												<td>
													{lowerLimit === undefined || lowerLimit === null
														? ""
														: (() => {
																const lower = `${
																	alert?.limit === BoundaryLimit.Lower
																}${formatNumber(lowerLimit)} (${formatNumber(
																	lowerLimitPercentage,
																)}%)`;
																return alert?.limit === BoundaryLimit.Lower ? (
																	<b>{lower}</b>
																) : (
																	lower
																);
															})()}
												</td>
												<td>
													{upperLimit === undefined || upperLimit === null
														? ""
														: (() => {
																const upper = `${formatNumber(
																	upperLimit,
																)} (${formatNumber(upperLimitPercentage)}%)`;
																return alert?.limit === BoundaryLimit.Upper ? (
																	<b>{upper}</b>
																) : (
																	upper
																);
															})()}
												</td>
											</tr>
										);
									}}
								</For>
							</tbody>
						</table>
					</div>
					<hr />
				</Show>
				<For each={props.value()?.results}>
					{(iteration) => (
						<div class="table-container">
							<table class="table is-bordered is-fullwidth">
								<thead>
									<tr>
										<th>Benchmark</th>
										<th>Latency (ns)</th>
										<th>
											Latency Results
											<br />
											nanoseconds (ns) | (Δ%)
										</th>
										<th>
											Latency Upper Boundary
											<br />
											nanoseconds (ns) | (%)
										</th>
									</tr>
								</thead>
								<tbody>
									<For each={iteration}>
										{(result) => (
											<tr>
												<td>
													<a
														href={`${resourcePath(props.isConsole)}/${
															props.params?.project
														}/benchmarks/${
															result?.benchmark?.slug
														}?${BACK_PARAM}=${encodePath()}`}
													>
														{result?.benchmark?.name}
													</a>
												</td>
												<For each={result?.measures}>
													{(report_measure) => (
														<td>{report_measure?.metric?.value}</td>
													)}
												</For>
											</tr>
										)}
									</For>
								</tbody>
							</table>
						</div>
					)}
				</For>
			</div>
		</div>
	);
};

// 30 days
const DEFAULT_ALERT_HISTORY = 30 * 24 * 60 * 60 * 1000;

export const alertPerfUrl = (
	isConsole: undefined | boolean,
	project: undefined | string,
	alert: JsonAlert,
) => {
	const start_time = dateTimeMillis(alert?.created);
	const perfQuery = {
		branches: alert?.threshold?.branch?.uuid,
		testbeds: alert?.threshold?.testbed?.uuid,
		benchmarks: alert?.benchmark?.uuid,
		measures: alert?.threshold?.measure?.uuid,
		lower_boundary: alert?.limit === BoundaryLimit.Lower,
		upper_boundary: alert?.limit === BoundaryLimit.Upper,
		start_time: start_time ? start_time - DEFAULT_ALERT_HISTORY : null,
		end_time: dateTimeMillis(alert?.modified),
	};

	const searchParams = new URLSearchParams();
	for (const [key, value] of Object.entries(perfQuery)) {
		if (value) {
			searchParams.set(key, value.toString());
		}
	}
	return `${
		isConsole ? `/console/projects/${project}/perf` : `/perf/${project}`
	}?${searchParams.toString()}`;
};

const alertUrl = (
	isConsole: undefined | boolean,
	project: undefined | string,
	alert: JsonReport,
) => {
	return `${resourcePath(isConsole)}/${project}/alerts/${
		alert?.uuid
	}?${BACK_PARAM}=${encodePath()}`;
};

const thresholdUrl = (
	isConsole: undefined | boolean,
	project: undefined | string,
	threshold: JsonThreshold,
) => {
	return `${resourcePath(isConsole)}/${project}/thresholds/${
		threshold?.uuid
	}?model=${threshold?.model?.uuid}&${BACK_PARAM}=${encodePath()}`;
};

// biome-ignore lint/style/useEnumInitializers: variants
enum Position {
	Whole,
	Point,
	Decimal,
}

const formatNumber = (number: number): string => {
	let numberStr = "";
	let position = Position.Decimal;
	const formattedNumber = number.toFixed(2);

	for (let i = formattedNumber.length - 1; i >= 0; i--) {
		const c = formattedNumber[i];
		switch (position) {
			case Position.Whole:
				if (
					(formattedNumber.length - 1 - i) % 3 === 0 &&
					i !== formattedNumber.length - 1
				) {
					numberStr = `,${numberStr}`;
				}
				position = Position.Whole;
				break;
			case Position.Point:
				position = Position.Whole;
				break;
			case Position.Decimal:
				if (c === ".") {
					position = Position.Point;
				}
				break;
		}
		numberStr = c + numberStr;
	}

	if (number < 0) {
		numberStr = `-${numberStr}`;
	}

	return numberStr;
};

export default TableCard;
