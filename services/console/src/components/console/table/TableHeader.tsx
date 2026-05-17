import * as Sentry from "@sentry/astro";
import type { Params } from "astro";
import {
	type Accessor,
	For,
	Match,
	Show,
	Switch,
	createEffect,
	createMemo,
	createResource,
	createSignal,
} from "solid-js";
import { Button } from "../../../config/types";
import { AlertStatus, type JsonAlert } from "../../../types/bencher";
import {
	activeAlertCount,
	fetchActiveAlertCount,
	invalidateActiveAlertCount,
} from "../../../util/active_alerts";
import { authUser } from "../../../util/auth";
import { X_TOTAL_COUNT, httpGet, httpPatch } from "../../../util/http";
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
	revoked: Accessor<undefined | string>;
	handleRefresh: () => void;
	handleStartTime: (start_time: string) => void;
	handleEndTime: (end_time: string) => void;
	handleSearch: (search: string) => void;
	handleArchived: () => void;
	handleRevoked: () => void;
}

export interface TableHeaderConfig {
	title: string;
	name?: string;
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
							revoked={props.revoked}
							title={title}
							name={props.config?.name}
							button={button}
							handleRefresh={props.handleRefresh}
							handleStartTime={props.handleStartTime}
							handleEndTime={props.handleEndTime}
							handleSearch={props.handleSearch}
							handleArchived={props.handleArchived}
							handleRevoked={props.handleRevoked}
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
	revoked: Accessor<undefined | string>;
	title: string;
	name: undefined | string;
	button: TableButton;
	handleRefresh: () => void;
	handleStartTime: (start_time: string) => void;
	handleEndTime: (end_time: string) => void;
	handleSearch: (search: string) => void;
	handleArchived: () => void;
	handleRevoked: () => void;
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
							placeholder: `Search ${props.name}`,
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
				<Match when={props.button.kind === Button.REVOKED}>
					<button
						class={`button${props.revoked() === "true" ? " is-primary" : ""}`}
						type="button"
						title={
							props.revoked() === "true"
								? `View active ${props.title}`
								: `View revoked ${props.title}`
						}
						onMouseDown={(e) => {
							e.preventDefault();
							props.handleRevoked();
						}}
					>
						<span class="icon">
							<i class="fas fa-ban" />
						</span>
						<span>Revoked</span>
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
	const [dismissing, setDismissing] = createSignal(false);

	createEffect(() => {
		const project = props.params.project;
		const token = authUser()?.token;
		if (!token || !project) {
			return;
		}
		fetchActiveAlertCount(props.apiUrl, project, token);
	});

	const anyActive = createMemo(() => {
		const project = props.params.project;
		if (!project) {
			return false;
		}
		return (activeAlertCount(project) ?? 0) > 0;
	});

	const dismissAll = async () => {
		const token = authUser()?.token;
		const project = props.params.project;
		if (!token || !project) {
			return;
		}
		setDismissing(true);
		try {
			const all_alerts: JsonAlert[] = [];
			const PER_PAGE = 255;
			let page = 1;
			while (true) {
				const searchParams = new URLSearchParams();
				searchParams.set(PER_PAGE_PARAM, PER_PAGE.toString());
				searchParams.set(PAGE_PARAM, page.toString());
				searchParams.set(ARCHIVED_PARAM, props.archived() ?? "false");
				const path = `/v0/projects/${project}/alerts?${searchParams.toString()}`;
				const resp = await httpGet(props.apiUrl, path, token).catch((error) => {
					console.error(error);
					Sentry.captureException(error);
					return null;
				});
				if (!resp) {
					break;
				}
				const total = resp.headers?.[X_TOTAL_COUNT];
				const alerts = resp.data;
				if (!total || !alerts) {
					break;
				}
				all_alerts.push(...alerts);
				if (alerts.length < PER_PAGE || all_alerts.length === total) {
					break;
				}
				page++;
			}
			const activeAlerts = all_alerts.filter(
				(alert) => alert.status === AlertStatus.Active,
			);
			let count = 0;
			for (const alert of activeAlerts) {
				try {
					await httpPatch(
						props.apiUrl,
						`/v0/projects/${project}/alerts/${alert.uuid}`,
						token,
						{ status: AlertStatus.Dismissed },
					);
					count++;
				} catch (error) {
					console.error(error);
					Sentry.captureException(error);
					const message =
						(error as { response?: { data?: { message?: string } } })?.response
							?.data?.message ?? "Unknown error";
					pageNotify(
						NotifyKind.ERROR,
						`Lettuce romaine calm! Failed to dismiss alerts: ${message}`,
					);
				}
			}
			if (count > 0) {
				invalidateActiveAlertCount(project);
				props.handleRefresh();
			}
		} finally {
			setDismissing(false);
		}
	};

	return (
		<button
			class={`button${dismissing() ? " is-loading" : ""}`}
			type="button"
			title={
				dismissing()
					? "Dismissing all alerts..."
					: anyActive()
						? "Dismiss all active alerts"
						: "No active alerts to dismiss"
			}
			disabled={!anyActive() || dismissing()}
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
