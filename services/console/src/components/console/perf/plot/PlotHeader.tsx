import {
	type Accessor,
	createEffect,
	createMemo,
	createResource,
	createSignal,
	For,
	Show,
} from "solid-js";
import type { JsonAuthUser, JsonMetricKind } from "../../../../types/bencher";
import { PerfRange } from "../../../../config/types";
import { httpGet } from "../../../../util/http";

const BENCHER_METRIC_KIND = "--bencher--metric--kind--";

export interface Props {
	apiUrl: string;
	user: JsonAuthUser;
	project_slug: Accessor<undefined | string>;
	isConsole: boolean;
	isPlotInit: Accessor<boolean>;
	metric_kind: Accessor<undefined | string>;
	start_date: Accessor<undefined | string>;
	end_date: Accessor<undefined | string>;
	refresh: () => void;
	range: Accessor<PerfRange>;
	clear: Accessor<boolean>;
	lower_value: Accessor<boolean>;
	upper_value: Accessor<boolean>;
	lower_boundary: Accessor<boolean>;
	upper_boundary: Accessor<boolean>;
	handleMetricKind: (metric_kind: null | string) => void;
	handleStartTime: (start_time: string) => void;
	handleEndTime: (end_time: string) => void;
	handleRange: (range: PerfRange) => void;
	handleClear: (clear: boolean) => void;
	handleLowerValue: (lower_value: boolean) => void;
	handleUpperValue: (upper_value: boolean) => void;
	handleLowerBoundary: (lower_boundary: boolean) => void;
	handleUpperBoundary: (upper_boundary: boolean) => void;
}

const PlotHeader = (props: Props) => {
	const metric_kinds_fetcher = createMemo(() => {
		return {
			project: props.project_slug(),
			refresh: props.refresh(),
			token: props.user?.token,
		};
	});
	const getMetricKinds = async (fetcher: {
		project: undefined | string;
		refresh: () => void;
		token: undefined | string;
	}) => {
		const SELECT_METRIC_KIND = {
			name: "Select Metric Kind",
			slug: BENCHER_METRIC_KIND,
		};
		if (!fetcher.project) {
			return [SELECT_METRIC_KIND];
		}
		if (props.isConsole && typeof fetcher.token !== "string") {
			return [SELECT_METRIC_KIND];
		}
		// Always use the first page and the max number of results per page
		const searchParams = new URLSearchParams();
		searchParams.set("per_page", "255");
		searchParams.set("page", "1");
		const path = `/v0/projects/${
			fetcher.project
		}/metric-kinds?${searchParams.toString()}`;
		return await httpGet(props.apiUrl, path, fetcher.token)
			.then((resp) => {
				let data = resp?.data;
				data.push(SELECT_METRIC_KIND);
				return data;
			})
			.catch((error) => {
				console.error(error);
				return [SELECT_METRIC_KIND];
			});
	};
	const [metric_kinds] = createResource<JsonMetricKind[]>(
		metric_kinds_fetcher,
		getMetricKinds,
	);

	const getSelected = () => {
		const slug = props.metric_kind();
		if (slug) {
			return slug;
		} else {
			return BENCHER_METRIC_KIND;
		}
	};
	const [selected, setSelected] = createSignal(getSelected());

	createEffect(() => {
		const slug = props.metric_kind();
		if (slug) {
			setSelected(slug);
		} else {
			setSelected(BENCHER_METRIC_KIND);
		}
	});

	const handleInput = (slug: string) => {
		if (slug === BENCHER_METRIC_KIND) {
			props.handleMetricKind(null);
		} else {
			props.handleMetricKind(slug);
		}
	};

	const range_icon = createMemo(() => {
		switch (props.range()) {
			case PerfRange.DATE_TIME:
				return <i class="far fa-calendar" aria-hidden="true" />;
			case PerfRange.VERSION:
				return <i class="fas fa-code-branch" aria-hidden="true" />;
		}
	});

	return (
		<nav class="panel-heading level">
			<div class="level-left">
				<div class="level-item">
					<div class="columns">
						<div class="column">
							<p>Metric Kind</p>
							<div class="columns">
								<div class="column">
									<select
										class="card-header-title level-item"
										title="Select Metric Kind"
										onInput={(e) => handleInput(e.currentTarget.value)}
									>
										<For each={metric_kinds() ?? []}>
											{(metric_kind: { name: string; slug: string }) => (
												<option
													value={metric_kind.slug}
													selected={metric_kind.slug === selected()}
												>
													{metric_kind.name}
												</option>
											)}
										</For>
									</select>
								</div>
							</div>
						</div>
					</div>
				</div>
			</div>
			<div class="level-right">
				<Show when={!props.isPlotInit()} fallback={<></>}>
					<>
						<div class="level-item">
							<div class="columns">
								<div class="column">
									<div
										class="icon-text"
										data-tooltip="Display lower/upper Metric values"
									>
										<span>⠀Value</span>
										<span class="icon">
											<i class="fas fa-info-circle" aria-hidden="true" />
										</span>
										<span>⠀</span>
									</div>
									<div class="columns">
										<div class="column">
											<nav class="level is-mobile">
												<div class="level-item">
													<LineArrowButton
														param_key={props.lower_value}
														handleParamKey={props.handleLowerValue}
														position="Lower Value"
														arrow="down"
													/>
													<LineArrowButton
														param_key={props.upper_value}
														handleParamKey={props.handleUpperValue}
														position="Upper Value"
														arrow="up"
													/>
												</div>
											</nav>
										</div>
									</div>
								</div>
							</div>
						</div>
						<div class="level-item">
							<div class="columns">
								<div class="column">
									<div
										class="icon-text"
										data-tooltip="Display lower/upper Threshold Boundary Limits"
									>
										<span>Boundary</span>
										<span class="icon">
											<i class="fas fa-info-circle" aria-hidden="true" />
										</span>
									</div>
									<div class="columns">
										<div class="column">
											<nav class="level is-mobile">
												<div class="level-item">
													<LineArrowButton
														param_key={props.lower_boundary}
														handleParamKey={props.handleLowerBoundary}
														position="Lower Boundary"
														arrow="down"
													/>
													<LineArrowButton
														param_key={props.upper_boundary}
														handleParamKey={props.handleUpperBoundary}
														position="Upper Boundary"
														arrow="up"
													/>
												</div>
											</nav>
										</div>
									</div>
								</div>
							</div>
						</div>
						<div class="level-item">
							<div class="level-item">
								<div class="columns">
									<div class="column">
										<div
											class="icon-text"
											data-tooltip="Toggle X-Axis between Date and Branch Version"
										>
											<span>⠀X-Axis</span>
											<span class="icon">
												<i class="fas fa-info-circle" aria-hidden="true" />
											</span>
											<span>⠀</span>
										</div>
										<div class="columns">
											<div class="column">
												<button
													class="button is-outlined is-fullwidth"
													title={
														props.range() === PerfRange.DATE_TIME
															? "Switch to Version Range"
															: "Switch to Date Range"
													}
													onClick={(e) => {
														e.preventDefault();
														switch (props.range()) {
															case PerfRange.DATE_TIME:
																props.handleRange(PerfRange.VERSION);
																break;
															case PerfRange.VERSION:
																props.handleRange(PerfRange.DATE_TIME);
																break;
														}
													}}
												>
													<span class="icon">{range_icon()}</span>
												</button>
											</div>
										</div>
									</div>
								</div>
							</div>
						</div>
					</>
				</Show>
				<div class="level-item">
					<nav class="level is-mobile">
						<div class="level-item">
							<div class="columns">
								<div class="column">
									<p>Start Date</p>
									<div class="columns">
										<div class="column">
											<input
												title="Start Date"
												type="date"
												value={props.start_date() ?? ""}
												onInput={(e) =>
													props.handleStartTime(e.currentTarget?.value)
												}
											/>
										</div>
									</div>
								</div>
							</div>
						</div>
						<div class="level-item">
							<div class="columns">
								<div class="column">
									<p>End Date</p>
									<div class="columns">
										<div class="column">
											<input
												title="End Date"
												type="date"
												value={props.end_date() ?? ""}
												onInput={(e) =>
													props.handleEndTime(e.currentTarget?.value)
												}
											/>
										</div>
									</div>
								</div>
							</div>
						</div>
					</nav>
				</div>
				<Show when={!props.isPlotInit()} fallback={<></>}>
					<div class="level-item">
						<div class="columns">
							<div class="column">
								<p>⠀Clear⠀</p>
								<div class="columns">
									<div class="column">
										<button
											class="button is-outlined is-fullwidth"
											title="Clear Query"
											onClick={(e) => {
												e.preventDefault();
												props.handleClear(true);
											}}
										>
											<span class="icon">
												<i class="fas fa-times-circle" aria-hidden="true" />
											</span>
										</button>
									</div>
								</div>
							</div>
						</div>
					</div>
				</Show>
			</div>
		</nav>
	);
};

const LineArrowButton = (props: {
	param_key: Accessor<boolean>;
	handleParamKey: (param_key: boolean) => void;
	position: string;
	arrow: string;
}) => {
	return (
		<button
			class={`button ${
				props.param_key() ? "is-primary" : "is-outlined"
			} is-fullwidth`}
			title={`${props.param_key() ? "Hide" : "Show"} ${props.position}`}
			onClick={(e) => {
				e.preventDefault();
				props.handleParamKey(!props.param_key());
			}}
		>
			<span class="icon">
				<i class={`fas fa-arrow-${props.arrow}`} aria-hidden="true" />
			</span>
		</button>
	);
};
export default PlotHeader;
