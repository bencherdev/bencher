import { useLocation, useNavigate } from "solid-app-router";
import { For, Match, Switch, createEffect, createMemo } from "solid-js";
import { concat_values, pageTitle } from "../../../site/util";
import { Button } from "../../config/types";
import StatusButton from "./StatusButton";
import PerfButton from "./PerfButton";

const DeckHeader = (props) => {
	const navigate = useNavigate();
	const location = useLocation();
	const pathname = createMemo(() => location.pathname);

	const title = createMemo(() =>
		concat_values(props.data(), props.config?.key, props.config?.keys, " | "),
	);

	createEffect(() => {
		pageTitle(title());
	});

	return (
		<div class="columns is-centered">
			<div class="column is-narrow">
				<button
					class="button is-outlined is-fullwidth"
					title={`Back to ${props.config?.path_to}`}
					onClick={(e) => {
						e.preventDefault();
						navigate(props.config?.path(pathname()));
					}}
				>
					<span class="icon">
						<i class="fas fa-chevron-left" aria-hidden="true" />
					</span>
					<span>Back</span>
				</button>
			</div>
			<div class="column">
				<div class="content has-text-centered">
					<h3 class="title is-3" style="overflow-wrap:anywhere;">
						{title()}
					</h3>
				</div>
			</div>

			<div class="column is-narrow">
				<nav class="level">
					<div class="level-right">
						<For each={props.config?.buttons}>
							{(button) => (
								<div class="level-item">
									<DeckHeaderButton
										title={title}
										user={props.user}
										data={props.data}
										url={props.url}
										button={button}
										path_params={props.path_params}
										handleRefresh={props.handleRefresh}
									/>
								</div>
							)}
						</For>
					</div>
				</nav>
			</div>
		</div>
	);
};

const DeckHeaderButton = (props) => {
	const navigate = useNavigate();
	const location = useLocation();
	const pathname = createMemo(() => location.pathname);

	return (
		<Switch fallback={<></>}>
			<Match when={props.button.kind === Button.EDIT}>
				<button
					class="button is-outlined is-fullwidth"
					title={`Edit ${props.title()}`}
					onClick={(e) => {
						e.preventDefault();
						navigate(props.button?.path(pathname()));
					}}
				>
					<span class="icon">
						<i class="fas fa-pen" aria-hidden="true" />
					</span>
					<span>Edit</span>
				</button>
			</Match>
			<Match when={props.button.kind === Button.STATUS}>
				<StatusButton
					user={props.user}
					data={props.data}
					url={props.url}
					handleRefresh={props.handleRefresh}
				/>
			</Match>
			<Match when={props.button.kind === Button.PERF}>
				<PerfButton data={props.data} path_params={props.path_params} />
			</Match>
			<Match when={props.button.kind === Button.REFRESH}>
				<button
					class="button is-outlined is-fullwidth"
					title={`Refresh ${props.title()}`}
					onClick={(e) => {
						e.preventDefault();
						props.handleRefresh();
					}}
				>
					<span class="icon">
						<i class="fas fa-sync-alt" aria-hidden="true" />
					</span>
					<span>Refresh</span>
				</button>
			</Match>
		</Switch>
	);
};

export default DeckHeader;
