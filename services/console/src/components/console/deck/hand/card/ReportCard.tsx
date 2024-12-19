import type { Params } from "astro";
import {
	type Accessor,
	For,
	Match,
	type Resource,
	Show,
	Switch,
	createMemo,
} from "solid-js";
import { perfPath, resourcePath } from "../../../../../config/util";
import {
	AlertStatus,
	BoundaryLimit,
	type JsonAlert,
	type JsonBenchmark,
	type JsonBranch,
	type JsonMeasure,
	type JsonReport,
	type JsonReportIteration,
	type JsonReportResults,
	type JsonTestbed,
	type JsonThreshold,
} from "../../../../../types/bencher";
import { dateTimeMillis, prettyPrintFloat } from "../../../../../util/convert";
import { BACK_PARAM, encodePath } from "../../../../../util/url";

export interface Props {
	isConsole?: boolean;
	params: Params;
	value: Resource<JsonReport>;
	width: Accessor<number>;
}

const ReportCard = (props: Props) => {
	const multipleIterations = createMemo(
		() => (props.value()?.results?.length ?? 0) > 1,
	);

	const benchmarkCount = createMemo(() => {
		if (props.value()?.results?.length === 0) {
			return 0;
		}
		return (
			props
				.value()
				?.results?.reduce((acc, iteration) => acc + iteration.length, 0) ?? 0
		);
	});

	const measuresMissingThresholds = createMemo(() =>
		Array.from(missingThreshold(props.value()?.results)),
	);

	const hasLowerBoundaryAlert = createMemo(() =>
		props.value()?.alerts?.some((alert) => alert.limit === BoundaryLimit.Lower),
	);
	const hasUpperBoundaryAlert = createMemo(() =>
		props.value()?.alerts?.some((alert) => alert.limit === BoundaryLimit.Upper),
	);

	return (
		<div class="columns is-centered" style="margin-top: 1rem">
			<div class="column is-12">
				<div class="content">
					<Show when={!props.value.loading && benchmarkCount() === 0}>
						<h3 class="title is-3">⚠️ WARNING: No benchmarks found!</h3>
					</Show>
					<Show
						when={
							benchmarkCount() > 0 && measuresMissingThresholds().length > 0
						}
					>
						<h3 class="title is-3">⚠️ WARNING: No Threshold found!</h3>
						<p>Without a Threshold, no Alerts will ever be generated.</p>
						<ul>
							<For each={measuresMissingThresholds()}>
								{(measure) => (
									<li>
										<a
											href={`${resourcePath(props.isConsole)}/${
												props.params?.project
											}/measures/${measure.slug}?${BACK_PARAM}=${encodePath()}`}
										>
											{measure.name} ({measure.units})
										</a>
									</li>
								)}
							</For>
						</ul>
						<Show when={props.isConsole}>
							<a
								href={`/console/projects/${
									props.params?.project
								}/thresholds/add?${BACK_PARAM}=${encodePath()}`}
							>
								Click here to create a new Threshold
							</a>
							<br />
						</Show>
						<p>
							For more information, see{" "}
							<a href="https://bencher.dev/docs/explanation/thresholds/">
								the Threshold documentation
							</a>
						</p>
					</Show>
					<Show when={(props.value()?.alerts?.length ?? 0) > 0}>
						<h3 class="title is-3">
							🚨 {props.value()?.alerts?.length}{" "}
							{props.value()?.alerts?.length === 1 ? "Alert" : "Alerts"}
						</h3>
						<div
							class="table-container"
							style={`max-width: ${props.width()}px;`}
						>
							<table class="table is-bordered is-fullwidth">
								<thead>
									<tr>
										{multipleIterations() && <th>Iteration</th>}
										<th>Benchmark</th>
										<th>
											Measure
											<br />
											Units
										</th>
										<th>View</th>
										<th>
											Benchmark Result
											<br />
											(Result Δ%)
										</th>
										<Show when={hasLowerBoundaryAlert()}>
											<th>
												Lower Boundary
												<br />
												(Limit %)
											</th>
										</Show>
										<Show when={hasUpperBoundaryAlert()}>
											<th>
												Upper Boundary
												<br />
												(Limit %)
											</th>
										</Show>
									</tr>
								</thead>
								<tbody>
									<For each={props.value()?.alerts}>
										{(alert) => {
											const value = alert?.metric?.value;
											const baseline = alert?.boundary?.baseline;
											const lowerLimit = alert?.boundary?.lower_limit;
											const upperLimit = alert?.boundary?.upper_limit;

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
															<br />
															{alert?.threshold?.measure?.units}
														</a>
													</td>
													<td>
														📈{" "}
														<a
															href={alertPerfUrl(
																props.isConsole,
																props.params?.project,
																props.value()?.uuid as string,
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
															alert ({alertStatus(alert)})
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
													<ValueCell value={value} baseline={baseline} bold />
													<Show when={hasLowerBoundaryAlert()}>
														<LowerLimitCell
															value={value}
															lowerLimit={lowerLimit}
															bold={alert?.limit === BoundaryLimit.Lower}
														/>
													</Show>
													<Show when={hasUpperBoundaryAlert()}>
														<UpperLimitCell
															value={value}
															upperLimit={upperLimit}
															bold={alert?.limit === BoundaryLimit.Upper}
														/>
													</Show>
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
								<div
									class="table-container"
									style={`max-width: ${props.width()}px;`}
								>
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
																	{(boundaryLimits.lower ||
																		boundaryLimits.upper) && (
																		<>
																			Benchmark Result
																			<br />
																		</>
																	)}
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
																		report_measure.measure?.slug ===
																		measure.slug,
																);

																const value = reportMeasure?.metric?.value;
																const baseline =
																	reportMeasure?.boundary?.baseline;
																const lowerLimit =
																	reportMeasure?.boundary?.lower_limit;
																const upperLimit =
																	reportMeasure?.boundary?.upper_limit;

																const alert = props
																	.value()
																	?.alerts?.find(
																		(alert) =>
																			alert.benchmark?.slug ===
																				result.benchmark?.slug &&
																			alert.threshold?.measure?.slug ===
																				measure?.slug,
																	);

																return (
																	<>
																		<td>
																			{"📈"}{" "}
																			<a
																				href={perfUrl(
																					props.isConsole,
																					props.params?.project,
																					props.value()?.uuid as string,
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
																						view alert ({alertStatus(alert)})
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
																		<Show
																			when={typeof value === "number"}
																			fallback={<td />}
																		>
																			<ValueCell
																				value={value as number}
																				baseline={baseline}
																				bold={!!alert}
																			/>
																		</Show>
																		<Show when={boundaryLimits.lower}>
																			<LowerLimitCell
																				value={value as number}
																				lowerLimit={lowerLimit}
																				bold={
																					alert?.limit === BoundaryLimit.Lower
																				}
																			/>
																		</Show>
																		<Show when={boundaryLimits.upper}>
																			<UpperLimitCell
																				value={value as number}
																				upperLimit={upperLimit}
																				bold={
																					alert?.limit === BoundaryLimit.Upper
																				}
																			/>
																		</Show>
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
		</div>
	);
};

const ValueCell = (props: {
	value: number;
	baseline: null | undefined | number;
	bold: boolean;
}) => {
	const ValueCellInner = (props: {
		value: number;
		baseline: null | undefined | number;
	}) => {
		if (typeof props.value !== "number") {
			return <></>;
		}

		const percent =
			typeof props.baseline === "number"
				? props.value > 0 && props.baseline > 0
					? ((props.value - props.baseline) / props.baseline) * 100
					: 0.0
				: null;

		return (
			<>
				{prettyPrintFloat(props.value)}
				<Show when={percent !== null}>
					<br />({percent > 0.0 ? "+" : ""}
					{prettyPrintFloat(percent)}%)
				</Show>
			</>
		);
	};

	return (
		<td>
			<Show
				when={props.bold}
				fallback={
					<ValueCellInner value={props.value} baseline={props.baseline} />
				}
			>
				<b>
					<ValueCellInner value={props.value} baseline={props.baseline} />
				</b>
			</Show>
		</td>
	);
};

const LowerLimitCell = (props: {
	value: number;
	lowerLimit: null | undefined | number;
	bold: boolean;
}) => {
	if (typeof props.value !== "number" || typeof props.lowerLimit !== "number") {
		return <td />;
	}

	const percent =
		props.value > 0 && props.lowerLimit > 0
			? (props.lowerLimit / props.value) * 100
			: 0.0;

	return (
		<LimitCell limit={props.lowerLimit} percent={percent} bold={props.bold} />
	);
};

const UpperLimitCell = (props: {
	value: number;
	upperLimit: null | undefined | number;
	bold: boolean;
}) => {
	if (typeof props.value !== "number" || typeof props.upperLimit !== "number") {
		return <td />;
	}

	const percent =
		props.value > 0 && props.upperLimit > 0
			? (props.value / props.upperLimit) * 100
			: 0.0;

	return (
		<LimitCell limit={props.upperLimit} percent={percent} bold={props.bold} />
	);
};

const LimitCell = (props: {
	limit: number;
	percent: number;
	bold: boolean;
}) => {
	const LimitCellInner = (props: {
		limit: number;
		percent: number;
	}) => (
		<>
			{prettyPrintFloat(props.limit)}
			<br />({prettyPrintFloat(props.percent)}%)
		</>
	);

	return (
		<td>
			<Show
				when={props.bold}
				fallback={
					<LimitCellInner limit={props.limit} percent={props.percent} />
				}
			>
				<b>
					<LimitCellInner limit={props.limit} percent={props.percent} />
				</b>
			</Show>
		</td>
	);
};

// 30 days
const DEFAULT_ALERT_HISTORY = 30 * 24 * 60 * 60 * 1000;

export const alertPerfUrl = (
	isConsole: undefined | boolean,
	project: undefined | string,
	reportUuid: undefined | string,
	alert: JsonAlert,
) =>
	perfUrl(
		isConsole,
		project,
		reportUuid,
		alert?.threshold?.branch,
		alert?.threshold?.testbed,
		alert?.benchmark,
		alert?.threshold?.measure,
		alert?.limit === BoundaryLimit.Lower,
		alert?.limit === BoundaryLimit.Upper,
		alert?.created,
		alert?.created,
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

const alertStatus = (alert: JsonAlert) => {
	switch (alert.status) {
		case AlertStatus.Active:
			return "🔔";
		case AlertStatus.Dismissed:
		case AlertStatus.Silenced:
			return "🔕";
	}
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
	reportUuid: undefined | string,
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
		report: reportUuid,
		branches: branch?.uuid,
		heads: branch?.head?.uuid,
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
	return `${perfPath(isConsole, project)}?${searchParams.toString()}`;
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

const missingThreshold = (
	results: undefined | JsonReportResults,
): Set<Measure> => {
	if (!results) {
		return new Set();
	}
	const measuresMap: Map<string, Measure> = new Map();
	for (const iteration of results) {
		for (const result of iteration) {
			for (const report_measure of result.measures) {
				if (!report_measure.threshold) {
					const measure = {
						name: report_measure.measure?.name,
						slug: report_measure.measure?.slug,
						units: report_measure.measure?.units,
					};
					const key = JSON.stringify(measure);
					measuresMap.set(key, measure);
				}
			}
		}
	}
	return new Set(measuresMap.values());
};

export const boundaryLimitsMap = (
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
	return map;
};

const union = (lhs: BoundaryLimits, rhs: BoundaryLimits): BoundaryLimits => {
	return {
		lower: lhs.lower || rhs.lower,
		upper: lhs.upper || rhs.upper,
	};
};

export default ReportCard;
