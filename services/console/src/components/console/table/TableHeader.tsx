import { createResource, For, Match, Switch, type Accessor } from "solid-js";
import { pathname } from "../../../util/url";
import { Button } from "../../../config/types";
import type { Params } from "astro";
import Field from "../../field/Field";
import FieldKind from "../../field/kind";

export interface Props {
	apiUrl: string;
	params: Params;
	config: TableHeaderConfig;
	search: Accessor<string>;
	handleRefresh: () => void;
	handleSearch: (search: string) => void;
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
							search={props.search}
							title={title}
							button={button}
							handleRefresh={props.handleRefresh}
							handleSearch={props.handleSearch}
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
	search: Accessor<string>;
	title: string;
	button: TableButton;
	handleRefresh: () => void;
	handleSearch: (search: string) => void;
}) => {
	const [isAllowed] = createResource(props.params, (params) =>
		props.button.is_allowed?.(props.apiUrl, params),
	);

	return (
		<p class="level-item">
			<Switch>
				<Match when={props.button.kind === Button.SEARCH}>
					<Field
						kind={FieldKind.SEARCH}
						fieldKey="search"
						value={props.search() ?? ""}
						config={{
							placeholder: `Search ${props.title}`,
						}}
						handleField={(_key, search, _valid) => props.handleSearch(search)}
					/>
				</Match>
				<Match when={props.button.kind === Button.ADD}>
					<a
						class="button is-outlined"
						title={`Add ${props.button.title}`}
						href={props.button.path(pathname())}
					>
						<span class="icon">
							<i class="fas fa-plus" aria-hidden="true" />
						</span>
						<span>Add</span>
					</a>
				</Match>
				<Match when={props.button.kind === Button.INVITE && isAllowed()}>
					<a
						class="button is-outlined"
						title={`Invite to ${props.button.title}`}
						href={props.button.path(pathname())}
					>
						<span class="icon">
							<i class="fas fa-envelope" aria-hidden="true" />
						</span>
						<span>Invite</span>
					</a>
				</Match>
				<Match when={props.button.kind === Button.REFRESH}>
					<button
						class="button is-outlined"
						type="button"
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
