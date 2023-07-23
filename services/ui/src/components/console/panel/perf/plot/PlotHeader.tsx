import axios from "axios";
import {
	createEffect,
	createMemo,
	createResource,
	createSignal,
	For,
	Show,
} from "solid-js";
import { get_options, validate_jwt } from "../../../../site/util";
import { Range } from "../../../config/types";

const BENCHER_METRIC_KIND = "--bencher--metric--kind--";

const PlotHeader = (props) => {
	const metric_kinds_fetcher = createMemo(() => {
		return {
			project: props.path_params()?.project_slug,
			refresh: props.refresh(),
			token: props.user?.token,
		};
	});

	const getMetricKinds = async (fetcher) => {
		const SELECT_METRIC_KIND = {
			name: "Metric Kind",
			slug: BENCHER_METRIC_KIND,
		};
		if (props.is_console && !validate_jwt(fetcher.token)) {
			return [SELECT_METRIC_KIND];
		}
		// Always use the first page and the max number of results per page
		const search_params = new URLSearchParams();
		search_params.set("per_page", "255");
		search_params.set("page", "1");
		const url = `${props.config?.metric_kinds_url(
			fetcher.project,
		)}?${search_params.toString()}`;
		return await axios(get_options(url, fetcher.token))
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

	const [metric_kinds] = createResource(metric_kinds_fetcher, getMetricKinds);

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

	const handleInput = (e) => {
		const target_slug = e.currentTarget.value;
		if (target_slug === BENCHER_METRIC_KIND) {
			props.handleMetricKind(null);
			return;
		}

		props.handleMetricKind(target_slug);
	};

	const icon = createMemo(() => {
		switch (props.range()) {
			case Range.DATE_TIME:
				return <i class="far fa-calendar" aria-hidden="true" />;
			case Range.VERSION:
				return <i class="fas fa-code-branch" aria-hidden="true" />;
		}
	});

	return (
		<nav class="panel-heading level">
			<div class="level-left">
				<select
					class="card-header-title level-item"
					onInput={(e) => handleInput(e)}
				>
					<For each={metric_kinds()}>
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
			<div class="level-right">
				<div class="level-item">
					<nav class="level is-mobile">
						<div class="level-item has-text-centered">
							<p class="card-header-title">Start Date</p>
							<input
								type="date"
								value={props.start_date()}
								onInput={(e) => props.handleStartTime(e.currentTarget?.value)}
							/>
						</div>
					</nav>
				</div>
				<div class="level-item">
					<nav class="level is-mobile">
						<div class="level-item has-text-centered">
							<p class="card-header-title">End Date</p>
							<input
								type="date"
								value={props.end_date()}
								onInput={(e) => props.handleEndTime(e.currentTarget?.value)}
							/>
						</div>
					</nav>
				</div>
				<div class="level-item">
					<button
						class="button is-outlined "
						title={
							props.range() === Range.DATE_TIME
								? "Switch to Version Range"
								: "Switch to Date Range"
						}
						onClick={(e) => {
							e.preventDefault();
							switch (props.range()) {
								case Range.DATE_TIME:
									props.handleRange(Range.VERSION);
									break;
								case Range.VERSION:
									props.handleRange(Range.DATE_TIME);
									break;
							}
						}}
					>
						<span class="icon">{icon()}</span>
					</button>
				</div>
				<Show when={!props.is_plot_init()} fallback={<></>}>
					<div class="level-item">
						<button
							class="button is-outlined "
							title="Clear Perf Plot"
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
			</div>
		</nav>
	);
};

export default PlotHeader;
