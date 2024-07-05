import type { Params } from "astro";
import { Match, Show, Switch, createMemo, createResource } from "solid-js";
import { Display } from "../../../../../config/types";
import type CardConfig from "./CardConfig";
import { authUser } from "../../../../../util/auth";
import { httpGet } from "../../../../../util/http";
import type { JsonProject } from "../../../../../types/bencher";
import { BACK_PARAM, encodePath } from "../../../../../util/url";

export interface Props {
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
										<a
											href={`/console/projects/${
												props.params?.project
											}/branches/${
												props.value?.branch
											}?${BACK_PARAM}=${encodePath()}`}
										>
											Version Number: {props.value?.version?.number}
											<br />
											{props.value?.version?.hash && (
												<>Version Hash: {props.value?.version?.hash}</>
											)}
										</a>
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
