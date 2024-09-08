import type { Params } from "astro";
import type { JsonReport } from "../../../../../types/bencher";
import { createMemo, For, Show, type Resource } from "solid-js";
import { resourcePath } from "../../../../../config/util";
import { BACK_PARAM, encodePath } from "../../../../../util/url";
import { format } from "d3";

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
									<th>Benchmark</th>
									<th>Measure (units)</th>
									{multipleIterations() && <th>Iteration</th>}
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
												{multipleIterations() && <td>{alert?.iteration}</td>}
												<td>
													📈 <a>plot</a>
													<br />🚨 <a>alert</a>
													<br />🚷 <a>threshold</a>
												</td>
												<td>
													<b>
														{`${formatNumber(value)} (${
															valuePercent > 0.0 ? "+" : ""
														}${formatNumber(valuePercent)}%)`}
													</b>
												</td>
												<td>
													{lowerLimit === undefined || lowerLimit === null ? (
														""
													) : (
														<b>{`${formatNumber(lowerLimit)} (${formatNumber(
															lowerLimitPercentage,
														)}%)`}</b>
													)}
												</td>
												<td>
													{upperLimit === undefined || upperLimit === null ? (
														""
													) : (
														<b>
															{`${formatNumber(upperLimit)} (${formatNumber(
																upperLimitPercentage,
															)}%)`}
														</b>
													)}
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
					numberStr = "," + numberStr;
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
		numberStr = "-" + numberStr;
	}

	return numberStr;
};

export default TableCard;
