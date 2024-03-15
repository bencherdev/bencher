import {
	type Accessor,
	For,
	type Resource,
	Show,
	createEffect,
	createMemo,
	createResource,
	createSignal,
} from "solid-js";
import { PerfRange } from "../../../../config/types";
import type {
	JsonAuthUser,
	JsonMeasure,
	JsonProject,
} from "../../../../types/bencher";
import { BENCHER_WORDMARK } from "../../../../util/ext";
import { httpGet } from "../../../../util/http";
import { BENCHER_MEASURE_ID } from "./util";
import { BACK_PARAM, encodePath } from "../../../../util/url";

const BENCHER_MEASURE = "--bencher-measure--";

export interface Props {
	apiUrl: string;
	user: JsonAuthUser;
	project: Resource<JsonProject>;
	project_slug: Accessor<undefined | string>;
	isConsole: boolean;
	isEmbed: boolean;
	isPlotInit: Accessor<boolean>;
	measures: Accessor<string[]>;
	start_date: Accessor<undefined | string>;
	end_date: Accessor<undefined | string>;
	refresh: () => void;
	range: Accessor<PerfRange>;
	clear: Accessor<boolean>;
	lower_value: Accessor<boolean>;
	upper_value: Accessor<boolean>;
	lower_boundary: Accessor<boolean>;
	upper_boundary: Accessor<boolean>;
	embed_title: Accessor<undefined | string>;
	embed_header: Accessor<boolean>;
	handleMeasure: (measure: null | string) => void;
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
	return (
		<Show when={props.isEmbed} fallback={<FullPlotHeader {...props} />}>
			<EmbedPlotHeader {...props} />
		</Show>
	);
};

const FullPlotHeader = (props: Props) => {
	const measures_fetcher = createMemo(() => {
		return {
			project: props.project_slug(),
			refresh: props.refresh(),
			token: props.user?.token,
		};
	});
	const getMeasures = async (fetcher: {
		project: undefined | string;
		refresh: () => void;
		token: undefined | string;
	}) => {
		const SELECT_MEASURE = {
			name: "Select Measure",
			uuid: BENCHER_MEASURE,
		};
		if (!fetcher.project) {
			return [SELECT_MEASURE];
		}
		if (props.isConsole && typeof fetcher.token !== "string") {
			return [SELECT_MEASURE];
		}
		// Always use the first page and the max number of results per page
		const searchParams = new URLSearchParams();
		searchParams.set("per_page", "255");
		searchParams.set("page", "1");
		const path = `/v0/projects/${
			fetcher.project
		}/measures?${searchParams.toString()}`;
		return await httpGet(props.apiUrl, path, fetcher.token)
			.then((resp) => {
				const data = [SELECT_MEASURE, ...(resp?.data ?? [])];
				return data;
			})
			.catch((error) => {
				console.error(error);
				return [SELECT_MEASURE];
			});
	};
	const [measures] = createResource<JsonMeasure[]>(
		measures_fetcher,
		getMeasures,
	);
	const measure = createMemo(() =>
		measures()?.find((measure) => props.measures()?.includes(measure.uuid)),
	);

	const getSelected = () => {
		const uuid = props.measures()?.[0];
		if (uuid) {
			return uuid;
		}
		return BENCHER_MEASURE;
	};
	const [selected, setSelected] = createSignal(getSelected());

	createEffect(() => {
		const uuid = props.measures()?.[0];
		if (uuid) {
			setSelected(uuid);
		} else {
			setSelected(BENCHER_MEASURE);
		}
	});

	const handleInput = (uuid: string) => {
		if (uuid === BENCHER_MEASURE) {
			props.handleMeasure(null);
		} else {
			props.handleMeasure(uuid);
		}
	};

	return (
		<nav class="panel-heading columns is-vcentered">
			<div class="column">
				<div class="level is-mobile" style="margin-bottom: 0.5rem;">
					<div class="level-left">
						<div class="level-item">
							<p id={BENCHER_MEASURE_ID} class="level-item">
								Measure
							</p>
							<Show when={props.isConsole}>
								<a
									class="level-item button is-small is-outlined is-rounded"
									title={`View ${measure()?.name}`}
									href={`/console/projects/${props.project_slug()}/measures/${
										measure()?.slug
									}?${BACK_PARAM}=${encodePath()}`}
								>
									<small>View</small>
								</a>
							</Show>
						</div>
					</div>
				</div>
				<select
					class="card-header-title level-item"
					title="Select Measure"
					onInput={(e) => handleInput(e.currentTarget.value)}
				>
					<For each={measures() ?? []}>
						{(measure: { name: string; uuid: string }) => (
							<option
								value={measure.uuid}
								selected={measure.uuid === selected()}
							>
								{measure.name}
							</option>
						)}
					</For>
				</select>
			</div>
			<SharedPlot {...props} />
		</nav>
	);
};

const EmbedPlotHeader = (props: Props) => {
	const perfUrl = createMemo(() => {
		const location = window.location;
		return `${location.protocol}//${location.hostname}${
			location.port ? `:${location.port}` : ""
		}/perf/${props.project()?.slug}/${location.search}`;
	});

	const title = createMemo(() => {
		const embedTitle = props.embed_title();
		switch (embedTitle) {
			case "":
				return <></>;
			default:
				return (
					<h1 class="title is-3" style="word-break: break-word;">
						{embedTitle ?? props.project()?.name}
					</h1>
				);
		}
	});

	return (
		<nav class="panel-heading">
			<div class="columns is-mobile is-centered is-vcentered is-gapless">
				<div class="column has-text-centered">
					{/* biome-ignore lint/a11y/noBlankTarget: internal */}
					<a href={perfUrl()} target="_blank">
						<img src={BENCHER_WORDMARK} width="128em" alt="🐰 Bencher" />
					</a>
					{title()}
				</div>
			</div>
			<Show when={props.embed_header()}>
				<div class="columns is-centered is-vcentered">
					<SharedPlot {...props} />
				</div>
			</Show>
		</nav>
	);
};

const SharedPlot = (props: Props) => {
	const range_icon = createMemo(() => {
		switch (props.range()) {
			case PerfRange.DATE_TIME:
				return <i class="far fa-calendar" aria-hidden="true" />;
			case PerfRange.VERSION:
				return <i class="fas fa-code-branch" aria-hidden="true" />;
		}
	});

	return (
		<>
			<Show when={!props.isPlotInit()}>
				<div class="column is-narrow">
					<div class="level is-mobile" style="margin-bottom: 0;">
						<div class="level-item">
							<div
								class={`icon-text ${props.isEmbed ? "has-tooltip-bottom" : ""}`}
								data-tooltip="Display lower/upper Metric values"
							>
								<span style="padding-left: 1em">Value</span>
								<span class="icon">
									<i class="fas fa-info-circle" aria-hidden="true" />
								</span>
								<span style="padding-right: 1em" />
							</div>
						</div>
					</div>
					<div class="level is-mobile">
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
					</div>
				</div>
				<div class="column is-narrow">
					<div class="level is-mobile" style="margin-bottom: 0;">
						<div class="level-item">
							<div
								class={`icon-text ${props.isEmbed ? "has-tooltip-bottom" : ""}`}
								data-tooltip="Display lower/upper Threshold Boundary Limits"
							>
								<span>Boundary</span>
								<span class="icon">
									<i class="fas fa-info-circle" aria-hidden="true" />
								</span>
							</div>
						</div>
					</div>
					<div class="level is-mobile">
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
					</div>
				</div>
				<div class="column is-narrow">
					<div class="level is-mobile" style="margin-bottom: 0;">
						<div class="level-item">
							<div
								class={`icon-text ${props.isEmbed ? "has-tooltip-bottom" : ""}`}
								data-tooltip="Toggle X-Axis between Date and Branch Version"
							>
								<span style="padding-left: 0.5em">X-Axis</span>
								<span class="icon">
									<i class="fas fa-info-circle" aria-hidden="true" />
								</span>
								<span style="padding-right: 0.5em" />
							</div>
						</div>
					</div>
					<button
						class="button is-outlined is-fullwidth"
						type="button"
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
			</Show>
			<div class="column is-narrow">
				<div class="level is-mobile">
					<div class="level-item">
						<div class="columns">
							<div class="column">
								<p>Start Date</p>
								<input
									title="Start Date"
									type="date"
									value={props.start_date() ?? ""}
									onInput={(e) => props.handleStartTime(e.currentTarget?.value)}
								/>
							</div>
						</div>
					</div>
					<div class="level-item">
						<div class="columns">
							<div class="column">
								<p>End Date</p>
								<input
									title="End Date"
									type="date"
									value={props.end_date() ?? ""}
									onInput={(e) => props.handleEndTime(e.currentTarget?.value)}
								/>
							</div>
						</div>
					</div>
				</div>
			</div>
			<Show when={!props.isPlotInit() && !props.isEmbed}>
				<div class="column is-narrow">
					<p
						class="has-text-centered"
						style="padding-left: 0.5em; padding-right: 0.5em;"
					>
						Clear
					</p>
					<button
						class="button is-outlined is-fullwidth"
						type="reset"
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
			</Show>
		</>
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
			type="button"
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
