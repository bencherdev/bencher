import type { Params } from "astro";
import { Match, Show, Switch, createMemo, createResource } from "solid-js";
import { Display } from "../../../../../config/types";
import type CardConfig from "./CardConfig";
import { authUser } from "../../../../../util/auth";
import { httpGet } from "../../../../../util/http";
import type { JsonBranch, JsonProject } from "../../../../../types/bencher";
import { BACK_PARAM, encodePath } from "../../../../../util/url";
import * as Sentry from "@sentry/astro";
import { fmtDateTime, resourcePath } from "../../../../../config/util";

export interface Props {
	isConsole?: boolean;
	apiUrl: string;
	params: Params;
	card: CardConfig;
	value: boolean | string | object;
	toggleUpdate: () => void;
}

const ViewCard = (props: Props) => {
	const [is_allowed] = createResource(props.params, (params) =>
		props.card?.is_allowed?.(props.apiUrl, params),
	);

	return (
		<form>
			<div id={props.card?.label} class="field is-horizontal">
				<div
					class={`field-label${(() => {
						switch (props.card?.display) {
							case Display.RAW:
							case Display.DATE_TIME:
							case Display.SWITCH:
							case Display.SELECT:
								return " is-normal";
							default:
								return "";
						}
					})()}`}
				>
					<label class="label">{props.card?.label}</label>
				</div>
				<div class="field-body">
					<div class="field is-expanded">
						<div class="control">
							<Show
								when={props.value}
								fallback={
									<input class="input is-static" type="text" readonly />
								}
							>
								<Switch>
									<Match when={props.card?.display === Display.RAW}>
										<input
											class="input is-static"
											type="text"
											placeholder={props.value}
											value={props.value}
											readonly
										/>
									</Match>
									<Match
										when={
											props.card?.display === Display.BRANCH ||
											props.card?.display === Display.TESTBED ||
											props.card?.display === Display.BENCHMARK ||
											props.card?.display === Display.MEASURE
										}
									>
										<a
											href={`${resourcePath(props.isConsole)}/${
												props.params?.project
											}/${(() => {
												switch (props.card?.display) {
													case Display.BRANCH:
														return "branches";
													case Display.TESTBED:
														return "testbeds";
													case Display.BENCHMARK:
														return "benchmarks";
													case Display.MEASURE:
														return "measures";
													default:
														return "";
												}
											})()}/${props.value?.slug}?${BACK_PARAM}=${encodePath()}`}
										>
											{props.value?.name}
										</a>
									</Match>
									<Match when={props.card?.display === Display.DATE_TIME}>
										<input
											class="input is-static"
											type="text"
											value={fmtDateTime(props.value)}
											readonly
										/>
									</Match>
									<Match when={props.card?.display === Display.SWITCH}>
										<input
											type="checkbox"
											class="switch"
											checked={
												typeof props.value === "boolean" ? props.value : false
											}
											disabled={true}
										/>
										<label />
									</Match>
									<Match when={props.card?.display === Display.SELECT}>
										{(() => {
											const value = props.card?.field?.value?.options.reduce(
												(field, option) => {
													if (props.value === option.value) {
														return option.option;
													}
													return field;
												},
												props.value,
											);
											return (
												<input
													class="input is-static"
													type="text"
													placeholder={value}
													value={value}
													readonly
												/>
											);
										})()}
									</Match>
									<Match when={props.card?.display === Display.START_POINT}>
										<StartPointCard {...props} />
									</Match>
									<Match when={props.card?.display === Display.GIT_HASH}>
										<GitHashCard {...props} />
									</Match>
								</Switch>
							</Show>
						</div>
					</div>
					<Show when={is_allowed()}>
						<div class="field">
							<div class="control">
								<button
									type="button"
									class="button"
									onMouseDown={(e) => {
										e.preventDefault();
										props.toggleUpdate();
									}}
								>
									Update
								</button>
							</div>
						</div>
					</Show>
				</div>
			</div>
		</form>
	);
};

const StartPointCard = (props: Props) => {
	const user = authUser();
	const branchFetcher = createMemo(() => {
		return {
			project_slug: props.params.project,
			branch: props.value?.branch,
			token: user?.token,
		};
	});
	const getBranch = async (fetcher: {
		project_slug: string;
		branch: string;
		refresh: number;
		token: string;
	}) => {
		const EMPTY_OBJECT = {};
		if (
			!fetcher.project_slug ||
			fetcher.project_slug === "undefined" ||
			!fetcher.branch
		) {
			return EMPTY_OBJECT;
		}
		const path = `/v0/projects/${fetcher.project_slug}/branches/${fetcher.branch}`;
		return await httpGet(props.apiUrl, path, fetcher.token)
			.then((resp) => {
				return resp?.data as JsonBranch;
			})
			.catch((error) => {
				console.error(error);
				Sentry.captureException(error);
				return EMPTY_OBJECT;
			});
	};
	const [branch] = createResource<JsonBranch>(branchFetcher, getBranch);

	return (
		<a
			href={`${linkPath(props.isConsole)}/${props.params?.project}/branches/${
				branch()?.slug ?? props.value?.branch
			}?${BACK_PARAM}=${encodePath()}`}
		>
			Branch: {branch()?.name}
			<br />
			Version Number: {props.value?.version?.number}
			<br />
			{props.value?.version?.hash && (
				<>Version Hash: {props.value?.version?.hash}</>
			)}
		</a>
	);
};

const GitHashCard = (props: Props) => {
	const user = authUser();
	const projectFetcher = createMemo(() => {
		return {
			project_slug: props.params.project,
			token: user?.token,
		};
	});
	const getProject = async (fetcher: {
		project_slug: string;
		refresh: number;
		token: string;
	}) => {
		const EMPTY_OBJECT = {};
		if (!fetcher.project_slug || fetcher.project_slug === "undefined") {
			return EMPTY_OBJECT;
		}
		const path = `/v0/projects/${fetcher.project_slug}`;
		return await httpGet(props.apiUrl, path, fetcher.token)
			.then((resp) => {
				return resp?.data as JsonProject;
			})
			.catch((error) => {
				console.error(error);
				Sentry.captureException(error);
				return EMPTY_OBJECT;
			});
	};
	const [project] = createResource<JsonProject>(projectFetcher, getProject);

	const hash = createMemo(() => {
		const url = project()?.url;
		if (url && isGitHubRepoUrl(url)) {
			return (
				<a href={`${url.endsWith("/") ? url : `${url}/`}commit/${props.value}`}>
					{props.value as string}
				</a>
			);
		}
		return <p style="word-break: break-word;">{props.value as string}</p>;
	});
	function isGitHubRepoUrl(url: string) {
		const regex = /^https:\/\/github\.com\/[a-zA-Z0-9_-]+\/[a-zA-Z0-9_-]+\/?$/;
		return regex.test(url);
	}

	return <>{hash()}</>;
};

export default ViewCard;
