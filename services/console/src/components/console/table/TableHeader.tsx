import * as Sentry from "@sentry/astro";
import type { Params } from "astro";
import {
	type Accessor,
	For,
	Match,
	type Resource,
	Show,
	Switch,
	createMemo,
	createResource,
} from "solid-js";
import { Button } from "../../../config/types";
import { AlertStatus, type JsonAlert } from "../../../types/bencher";
import { authUser } from "../../../util/auth";
import { X_TOTAL_COUNT, apiUrl, httpGet, httpPatch } from "../../../util/http";
import { NotifyKind, pageNotify } from "../../../util/notify";
import { pathname } from "../../../util/url";
import Field from "../../field/Field";
import FieldKind from "../../field/kind";
import DateRange from "../../field/kinds/DateRange";
import { ARCHIVED_PARAM, PAGE_PARAM, PER_PAGE_PARAM } from "./TablePanel";

export interface Props {
	apiUrl: string;
	params: Params;
	config: TableHeaderConfig;
	start_date: Accessor<undefined | string>;
	end_date: Accessor<undefined | string>;
	search: Accessor<undefined | string>;
	archived: Accessor<undefined | string>;
	handleRefresh: () => void;
	handleStartTime: (start_time: string) => void;
	handleEndTime: (end_time: string) => void;
	handleSearch: (search: string) => void;
	handleArchived: () => void;
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
							start_date={props.start_date}
							end_date={props.end_date}
							search={props.search}
							archived={props.archived}
							title={title}
							button={button}
							handleRefresh={props.handleRefresh}
							handleStartTime={props.handleStartTime}
							handleEndTime={props.handleEndTime}
							handleSearch={props.handleSearch}
							handleArchived={props.handleArchived}
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
	start_date: Accessor<undefined | string>;
	end_date: Accessor<undefined | string>;
	search: Accessor<undefined | string>;
	archived: Accessor<undefined | string>;
	title: string;
	button: TableButton;
	handleRefresh: () => void;
	handleStartTime: (start_time: string) => void;
	handleEndTime: (end_time: string) => void;
	handleSearch: (search: string) => void;
	handleArchived: () => void;
}) => {
	const [isAllowed] = createResource(props.params, (params) =>
		props.button.is_allowed?.(props.apiUrl, params),
	);

	return (
		<p class="level-item">
			<Switch>
				<Match when={props.button.kind === Button.DATE_TIME}>
					<div class="box">
						<DateRange
							start_date={props.start_date}
							end_date={props.end_date}
							handleStartTime={props.handleStartTime}
							handleEndTime={props.handleEndTime}
						/>
					</div>
				</Match>
				<Match when={props.button.kind === Button.SEARCH}>
					<Field
						kind={FieldKind.SEARCH}
						fieldKey="search"
						value={props.search() ?? ""}
						config={{
							placeholder: `Search ${props.title}`,
						}}
						handleField={(_key, search, _valid) =>
							props.handleSearch(search as string)
						}
					/>
				</Match>
				<Match when={props.button.kind === Button.DISMISS_ALL}>
					<DismissAllButton
						apiUrl={props.apiUrl}
						params={props.params}
						archived={props.archived}
						handleRefresh={props.handleRefresh}
					/>
				</Match>
				<Match when={props.button.kind === Button.ARCHIVED}>
					<button
						class={`button${props.archived() === "true" ? " is-primary" : ""}`}
						type="button"
						title={
							props.archived() === "true"
								? `View active ${props.title}`
								: `View archived ${props.title}`
						}
						onMouseDown={(e) => {
							e.preventDefault();
							props.handleArchived();
						}}
					>
						<span class="icon">
							<i class="fas fa-archive" />
						</span>
						<span>Archived</span>
					</button>
				</Match>
				<Match when={props.button.kind === Button.ADD}>
					<Show
						when={isAllowed.loading || isAllowed()}
						fallback={
							<button
								type="button"
								class="button"
								title={`Add ${props.button.title}`}
								disabled={true}
							>
								<span class="icon">
									<i class="fas fa-plus" />
								</span>
								<span>Add</span>
							</button>
						}
					>
						<a
							class="button"
							title={`Add ${props.button.title}`}
							href={props.button.path(pathname())}
						>
							<span class="icon">
								<i class="fas fa-plus" />
							</span>
							<span>Add</span>
						</a>
					</Show>
				</Match>
				<Match when={props.button.kind === Button.INVITE}>
					<Show
						when={isAllowed()}
						fallback={
							<button
								type="button"
								class="button"
								title={`Invite to ${props.button.title}`}
								disabled={true}
							>
								<span class="icon">
									<i class="fas fa-envelope" />
								</span>
								<span>Invite</span>
							</button>
						}
					>
						<a
							class="button"
							title={`Invite to ${props.button.title}`}
							href={props.button.path(pathname())}
						>
							<span class="icon">
								<i class="fas fa-envelope" />
							</span>
							<span>Invite</span>
						</a>
					</Show>
				</Match>
				<Match when={props.button.kind === Button.REFRESH}>
					<button
						class="button"
						type="button"
						title={`Refresh ${props.title}`}
						onMouseDown={(e) => {
							e.preventDefault();
							props.handleRefresh();
						}}
					>
						<span class="icon">
							<i class="fas fa-sync-alt" />
						</span>
						<span>Refresh</span>
					</button>
				</Match>
			</Switch>
		</p>
	);
};

const DismissAllButton = (props: {
	apiUrl: string;
	params: Params;
	archived: Accessor<undefined | string>;
	handleRefresh: () => void;
}) => {
	const alertsQuery = async (
		apiUrl: string,
		params: Params,
		per_page: number,
		page: number,
		archived: boolean,
		token: string,
	) => {
		const searchParams = new URLSearchParams();
		searchParams.set(PER_PAGE_PARAM, per_page.toString());
		searchParams.set(PAGE_PARAM, page.toString());
		searchParams.set(ARCHIVED_PARAM, archived.toString());
		const path = `/v0/projects/${params.project}/alerts?${searchParams.toString()}`;
		return await httpGet(apiUrl, path, token)
			.then((resp) => {
				return [resp?.headers?.[X_TOTAL_COUNT], resp?.data];
			})
			.catch((error) => {
				console.error(error);
				Sentry.captureException(error);
				return [];
			});
	};
	const fetcher = createMemo(() => {
		return {
			apiUrl: props.apiUrl,
			params: props.params,
			archived: props.archived(),
			token: authUser()?.token,
		};
	});
	const getAlerts = async (fetcher: {
		apiUrl: string;
		params: Params;
		archived: undefined | boolean;
		token: string;
	}) => {
		const all_alerts: JsonAlert[] = [];
		if (!fetcher?.token) {
			return all_alerts;
		}
		const PER_PAGE = 255;
		let page = 1;
		while (true) {
			const [total, alerts] = await alertsQuery(
				fetcher.apiUrl,
				fetcher.params,
				PER_PAGE,
				page,
				fetcher.archived ?? false,
				fetcher.token,
			);
			if (!total || !alerts) {
				break;
			}
			all_alerts.push(...alerts);
			if (alerts.length < PER_PAGE || all_alerts.length === total) {
				break;
			}
			page++;
		}
		// console.log(all_alerts);
		return all_alerts;
	};
	const [alerts] = createResource<JsonAlert[]>(fetcher, getAlerts);

	const anyActive = createMemo(() =>
		alerts()?.some((alert) => alert.status === AlertStatus.Active),
	);

	const dismissAll = () => {
		const fetch = fetcher();
		if (!fetch.token) {
			return;
		}
		const activeAlerts = alerts()?.filter(
			(alert) => alert.status === AlertStatus.Active,
		);
		let count = 0;
		for (const alert of activeAlerts) {
			httpPatch(
				fetch.apiUrl,
				`/v0/projects/${fetch.params.project}/alerts/${alert.uuid}`,
				fetch.token,
				{ status: AlertStatus.Dismissed },
			)
				.then((_resp) => {
					count++;
					if (count === activeAlerts.length) {
						if (props.archived() === "true") {
							props.handleRefresh();
						} else {
							// TODO move to global state
							// Reload the entire page to update the alert count in the side bar
							window.location.reload();
						}
					}
				})
				.catch((error) => {
					console.error(error);
					Sentry.captureException(error);
					pageNotify(
						NotifyKind.ERROR,
						"Lettuce romaine calm! Failed to dismiss alerts. Please, try again.",
					);
				});
		}
	};

	return (
		<button
			class="button"
			type="button"
			title={
				anyActive()
					? "Dismiss all active alerts"
					: "No active alerts to dismiss"
			}
			disabled={!anyActive()}
			onMouseDown={(e) => {
				e.preventDefault();
				dismissAll();
			}}
		>
			<Show
				when={anyActive()}
				fallback={
					<span class="icon">
						<i class="far fa-bell-slash" />
					</span>
				}
			>
				<span class="icon has-text-primary">
					<i class="far fa-bell" />
				</span>
			</Show>
			<span>Dismiss All</span>
		</button>
	);
};

export default TableHeader;
