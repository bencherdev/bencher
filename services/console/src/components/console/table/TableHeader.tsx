import { createResource, For, Match, Switch } from "solid-js";
import { pathname, useNavigate } from "../../../util/url";
import { Button } from "../../../config/types";
import type { Params } from "astro";

export interface Props {
	apiUrl: string;
	params: Params;
	config: TableHeaderConfig;
	handleRefresh: () => void;
}

export interface TableHeaderConfig {
	title: string;
	buttons: TableButton[];
}

const TableHeader = (props: Props) => {
	const title = props.config?.title;

	return (
		<nav class="level">
			<div class="level-left">
				<div class="level-item">
					<h3 class="title is-3">{title}</h3>
				</div>
			</div>

			<div class="level-right">
				<For each={props.config?.buttons}>
					{(button) => (
						<TableHeaderButton
							apiUrl={props.apiUrl}
							params={props.params}
							title={title}
							button={button}
							handleRefresh={props.handleRefresh}
						/>
					)}
				</For>
			</div>
		</nav>
	);
};

interface TableButton {
	title: string;
	kind: Button;
	is_allowed?: (apiUrl: string, params: Params) => boolean;
	path: (pathname: string) => string;
}

const TableHeaderButton = (props: {
	apiUrl: string;
	params: Params;
	title: string;
	button: TableButton;
	handleRefresh: () => void;
}) => {
	const navigate = useNavigate();

	const [isAllowed] = createResource(props.params, (params) =>
		props.button.is_allowed?.(props.apiUrl, params),
	);

	return (
		<p class="level-item">
			<Switch fallback={<></>}>
				<Match when={props.button.kind === Button.ADD}>
					<button
						class="button is-outlined"
						title={`Add ${props.button.title}`}
						onClick={(e) => {
							e.preventDefault();
							navigate(props.button.path(pathname()));
						}}
					>
						<span class="icon">
							<i class="fas fa-plus" aria-hidden="true" />
						</span>
						<span>Add</span>
					</button>
				</Match>
				<Match when={props.button.kind === Button.INVITE && isAllowed()}>
					<button
						class="button is-outlined"
						title={`Invite to ${props.button.title}`}
						onClick={(e) => {
							e.preventDefault();
							navigate(props.button.path(pathname()));
						}}
					>
						<span class="icon">
							<i class="fas fa-envelope" aria-hidden="true" />
						</span>
						<span>Invite</span>
					</button>
				</Match>
				<Match when={props.button.kind === Button.REFRESH}>
					<button
						class="button is-outlined"
						title={`Refresh ${props.title}`}
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
		</p>
	);
};

export default TableHeader;
