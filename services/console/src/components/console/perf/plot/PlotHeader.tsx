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
import {
	XAxis,
	type JsonAuthUser,
	type JsonMeasure,
	type JsonProject,
} from "../../../../types/bencher";
import {
	BENCHER_WORDMARK,
	BENCHER_WORDMARK_DARK,
	BENCHER_WORDMARK_ID,
} from "../../../../util/ext";
import { httpGet } from "../../../../util/http";
import { BENCHER_MEASURE_ID } from "./util";
import { BACK_PARAM, encodePath } from "../../../../util/url";
import { Theme, getTheme, themeWordmark } from "../../../navbar/theme/theme";

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
	x_axis: Accessor<XAxis>;
	clear: Accessor<boolean>;
	lower_value: Accessor<boolean>;
	upper_value: Accessor<boolean>;
	lower_boundary: Accessor<boolean>;
	upper_boundary: Accessor<boolean>;
	embed_logo: Accessor<undefined | string>;
	embed_title: Accessor<undefined | string>;
	embed_header: Accessor<boolean>;
	handleMeasure: (measure: null | string) => void;
	handleStartTime: (start_time: string) => void;
	handleEndTime: (end_time: string) => void;
	handleXAxis: (x_axis: XAxis) => void;
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
							<Show
								when={
									props.isConsole &&
									measure() &&
									measure()?.uuid !== BENCHER_MEASURE
								}
							>
								<a
									class="level-item button is-small is-rounded"
									style="margin-left: 1rem;"
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
					style="color: black;"
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

	const logo = createMemo(() => {
		const embedLogo = props.embed_logo();
		if (embedLogo === "") {
			return <></>;
		}
		return (
			<a href={perfUrl()} target="_blank">
				<img
					src={themeWordmark(
						(() => {
							if (embedLogo === Theme.Light || embedLogo === Theme.Dark) {
								return embedLogo;
							}
							return getTheme();
						})(),
					)}
					width="128em"
					alt="🐰 Bencher"
				/>
			</a>
		);
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
					{logo()}
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
	const xAxisIcon = createMemo(() => {
		switch (props.x_axis()) {
			case XAxis.DateTime:
				return <i class="far fa-calendar" />;
			case XAxis.Version:
				return <i class="fas fa-code-branch" />;
		}
	});

	return (
		<>
			<Show when={!props.isPlotInit()}>
				<div class="column is-narrow">
					<div class="level is-mobile" style="margin-bottom: 0;">
						<div class="level-item" title="Display lower/upper Metric values">
							<span style="padding-left: 1em; padding-right: 1em">Value</span>
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
						<div
							class="level-item"
							title="Display lower/upper Threshold Boundary Limits"
						>
							Boundary
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
						<div
							class="level-item"
							title="Toggle X-Axis between Date and Branch Version"
						>
							X-Axis
						</div>
					</div>
					<button
						class="button is-fullwidth"
						type="button"
						title={(() => {
							switch (props.x_axis()) {
								case XAxis.DateTime:
									return "Switch X-Axis to Branch Version";
								case XAxis.Version:
									return "Switch X-Axis to Date";
							}
						})()}
						onClick={(e) => {
							e.preventDefault();
							switch (props.x_axis()) {
								case XAxis.DateTime:
									props.handleXAxis(XAxis.Version);
									break;
								case XAxis.Version:
									props.handleXAxis(XAxis.DateTime);
									break;
							}
						}}
					>
						<span class="icon">{xAxisIcon()}</span>
					</button>
				</div>
			</Show>
			<div class="column is-narrow">
				<div class="level is-mobile">
					<div class="level-item">
						<div class="columns">
							<div class="column">
								<p title="Select a start date">Start Date</p>
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
								<p title="Select an end date">End Date</p>
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
						title="Clear the current query"
					>
						Clear
					</p>
					<button
						class="button is-fullwidth"
						type="reset"
						title="Clear Query"
						onClick={(e) => {
							e.preventDefault();
							props.handleClear(true);
						}}
					>
						<span class="icon">
							<i class="fas fa-times-circle" />
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
			class={`button ${props.param_key() ? "is-primary" : ""} is-fullwidth`}
			type="button"
			title={`${props.param_key() ? "Hide" : "Show"} ${props.position}`}
			onClick={(e) => {
				e.preventDefault();
				props.handleParamKey(!props.param_key());
			}}
		>
			<span class="icon">
				<i class={`fas fa-arrow-${props.arrow}`} />
			</span>
		</button>
	);
};
export default PlotHeader;
