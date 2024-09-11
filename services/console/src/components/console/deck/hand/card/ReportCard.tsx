import type { Params } from "astro";
import {
	BoundaryLimit,
	type JsonAlert,
	type JsonBenchmark,
	type JsonBranch,
	type JsonMeasure,
	type JsonReport,
	type JsonReportIteration,
	type JsonTestbed,
	type JsonThreshold,
} from "../../../../../types/bencher";
import { createMemo, For, Match, Show, Switch, type Resource } from "solid-js";
import { resourcePath } from "../../../../../config/util";
import { BACK_PARAM, encodePath } from "../../../../../util/url";
import { dateTimeMillis } from "../../../../../util/convert";

export interface Props {
	isConsole?: boolean;
	apiUrl: string;
	params: Params;
	value: Resource<JsonReport>;
}

const ReportCard = (props: Props) => {
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
																const lower = `${formatNumber(
																	lowerLimit,
																)} (${formatNumber(lowerLimitPercentage)}%)`;
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
					{(iteration) => {
						const measureBoundaryLimits = createMemo(() =>
							Array.from(boundaryLimitsMap(iteration).entries()),
						);

						return (
							<div class="table-container">
								<table class="table is-bordered is-fullwidth">
									<thead>
										<tr>
											<th>Benchmark</th>
											<For each={measureBoundaryLimits()}>
												{(entry) => {
													const measure = JSON.parse(entry[0]);
													const boundaryLimits = entry[1];
													return (
														<>
															<th>
																<a
																	href={measureUrl(
																		props.isConsole,
																		props.params?.project,
																		measure,
																	)}
																>
																	{measure?.name}
																</a>
															</th>
															<th>
																{measure?.units}
																{(boundaryLimits.lower ||
																	boundaryLimits.upper) && (
																	<>
																		<br />
																		(Result Δ%)
																	</>
																)}
															</th>
															{boundaryLimits.lower && (
																<th>
																	Lower Boundary
																	<br />
																	{measure?.units}
																	<br />
																	(Limit %)
																</th>
															)}
															{boundaryLimits.upper && (
																<th>
																	Upper Boundary
																	<br />
																	{measure?.units}
																	<br />
																	(Limit %)
																</th>
															)}
														</>
													);
												}}
											</For>
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
													<For each={measureBoundaryLimits()}>
														{(entry) => {
															const measure = JSON.parse(entry[0]);
															const boundaryLimits = entry[1];

															const reportMeasure = result.measures.find(
																(report_measure) =>
																	report_measure.measure?.slug === measure.slug,
															);

															const value = reportMeasure?.metric?.value ?? 0;
															const baseline =
																reportMeasure?.boundary?.baseline ?? 0;
															const valuePercent =
																value > 0 && baseline > 0
																	? ((value - baseline) / baseline) * 100
																	: 0.0;
															const lowerLimit =
																reportMeasure?.boundary?.lower_limit;
															const upperLimit =
																reportMeasure?.boundary?.upper_limit;
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

															const alert = props
																.value()
																?.alerts?.find(
																	(alert) =>
																		alert.benchmark?.slug ===
																			result.benchmark?.slug &&
																		alert.threshold?.measure?.slug ===
																			measure?.slug,
																);

															const valueCell = (
																<>
																	{formatNumber(value)}
																	<Show when={reportMeasure?.threshold}>
																		{" "}
																		({valuePercent > 0.0 ? "+" : ""}
																		{formatNumber(valuePercent)}%)
																	</Show>
																</>
															);
															const lowerBoundaryCell = (
																<>
																	{formatNumber(lowerLimit ?? 0)} (
																	{formatNumber(lowerLimitPercentage)}%)
																</>
															);
															const upperBoundaryCell = (
																<>
																	{formatNumber(upperLimit ?? 0)} (
																	{formatNumber(upperLimitPercentage)}%)
																</>
															);

															return (
																<>
																	<td>
																		{"📈"}{" "}
																		<a
																			href={perfUrl(
																				props.isConsole,
																				props.params?.project,
																				props.value()?.branch as JsonBranch,
																				props.value()?.testbed as JsonTestbed,
																				result.benchmark,
																				reportMeasure?.measure as JsonMeasure,
																				boundaryLimits.lower,
																				boundaryLimits.upper,
																				props.value()?.start_time as string,
																				props.value()?.end_time as string,
																			)}
																		>
																			view plot
																		</a>
																		<Switch>
																			<Match when={alert}>
																				<br />
																				{"🚨"}{" "}
																				<a
																					href={alertUrl(
																						props.isConsole,
																						props.params?.project,
																						alert as JsonAlert,
																					)}
																				>
																					view alert
																				</a>
																				<br />
																				{"🚷"}{" "}
																				<a
																					href={thresholdUrl(
																						props.isConsole,
																						props.params?.project,
																						alert?.threshold as JsonThreshold,
																					)}
																				>
																					view threshold
																				</a>
																			</Match>
																			<Match when={reportMeasure?.threshold}>
																				<br />
																				{"🚷"}{" "}
																				<a
																					href={thresholdUrl(
																						props.isConsole,
																						props.params?.project,
																						reportMeasure?.threshold as JsonThreshold,
																					)}
																				>
																					view threshold
																				</a>
																			</Match>
																			<Match when={true}>
																				<br />
																				{"⚠️ NO THRESHOLD"}
																			</Match>
																		</Switch>
																	</td>
																	<td>
																		<Show when={alert} fallback={valueCell}>
																			<b>{valueCell}</b>
																		</Show>
																	</td>
																	{boundaryLimits.lower && (
																		<td>
																			<Show
																				when={
																					alert?.limit === BoundaryLimit.Lower
																				}
																				fallback={lowerBoundaryCell}
																			>
																				<b>{lowerBoundaryCell}</b>
																			</Show>
																		</td>
																	)}
																	{boundaryLimits.upper && (
																		<td>
																			<Show
																				when={
																					alert?.limit === BoundaryLimit.Upper
																				}
																				fallback={upperBoundaryCell}
																			>
																				<b>{upperBoundaryCell}</b>
																			</Show>
																		</td>
																	)}
																</>
															);
														}}
													</For>
												</tr>
											)}
										</For>
									</tbody>
								</table>
							</div>
						);
					}}
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
) =>
	perfUrl(
		isConsole,
		project,
		alert?.threshold?.branch,
		alert?.threshold?.testbed,
		alert?.benchmark,
		alert?.threshold?.measure,
		alert?.limit === BoundaryLimit.Lower,
		alert?.limit === BoundaryLimit.Upper,
		alert?.created,
		alert?.modified,
	);

const alertUrl = (
	isConsole: undefined | boolean,
	project: undefined | string,
	alert: JsonAlert,
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

const measureUrl = (
	isConsole: undefined | boolean,
	project: undefined | string,
	measure: Measure,
) => {
	return `${resourcePath(isConsole)}/${project}/measures/${
		measure.slug
	}?${BACK_PARAM}=${encodePath()}`;
};

export const perfUrl = (
	isConsole: undefined | boolean,
	project: undefined | string,
	branch: JsonBranch,
	testbed: JsonTestbed,
	benchmark: JsonBenchmark,
	measure: JsonMeasure,
	lowerBoundary: boolean,
	upperBoundary: boolean,
	startTime: string,
	endTime: string,
) => {
	const start_time = dateTimeMillis(startTime);
	const perfQuery = {
		branches: branch?.uuid,
		testbeds: testbed?.uuid,
		benchmarks: benchmark?.uuid,
		measures: measure?.uuid,
		lower_boundary: lowerBoundary,
		upper_boundary: upperBoundary,
		start_time: start_time ? start_time - DEFAULT_ALERT_HISTORY : null,
		end_time: dateTimeMillis(endTime),
	};

	const searchParams = new URLSearchParams();
	for (const [key, value] of Object.entries(perfQuery)) {
		if (value) {
			searchParams.set(key, value.toString());
		}
	}
	return `${
		isConsole === true
			? `/console/projects/${project}/perf`
			: `/perf/${project}`
	}?${searchParams.toString()}`;
};

type Measure = {
	name: string;
	slug: string;
	units: string;
};

type BoundaryLimits = {
	lower: boolean;
	upper: boolean;
};

const boundaryLimitsMap = (
	iteration: JsonReportIteration,
): Map<string, BoundaryLimits> => {
	const map = new Map<string, BoundaryLimits>();
	for (const result of iteration) {
		for (const report_measure of result.measures) {
			const measure = {
				name: report_measure.measure?.name,
				slug: report_measure.measure?.slug,
				units: report_measure.measure?.units,
			};
			const boundaryLimits = {
				lower: typeof report_measure.boundary?.lower_limit === "number",
				upper: typeof report_measure.boundary?.upper_limit === "number",
			};
			const measureKey = JSON.stringify(measure);
			const currentBoundaryLimits = map.get(measureKey);
			if (currentBoundaryLimits) {
				map.set(measureKey, union(currentBoundaryLimits, boundaryLimits));
			} else {
				map.set(measureKey, boundaryLimits);
			}
		}
	}
	console.log(map);
	return map;
};

const union = (lhs: BoundaryLimits, rhs: BoundaryLimits): BoundaryLimits => {
	return {
		lower: lhs.lower || rhs.lower,
		upper: lhs.upper || rhs.upper,
	};
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
	const formattedNumber = Math.abs(number).toFixed(2);
	const isNegative = number < 0;

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

	if (isNegative) {
		numberStr = `-${numberStr}`;
	}

	return numberStr;
};

export default ReportCard;
