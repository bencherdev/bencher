import type { Params } from "astro";
import type { JsonPlot, JsonProject } from "../../../types/bencher";
import {
	For,
	Show,
	createEffect,
	createMemo,
	createResource,
	type Accessor,
} from "solid-js";
import { setPageTitle } from "../../../util/resource";
import { authUser, isAllowedProjectManage } from "../../../util/auth";
import { httpGet } from "../../../util/http";
import Pinned from "./Pinned";

const MAX_PLOTS = 255;

export interface Props {
	apiUrl: string;
	params: Params;
	project?: undefined | JsonProject;
}

const DashboardPanel = (props: Props) => {
	const params = createMemo(() => props.params);
	const user = authUser();

	createEffect(() => {
		setPageTitle(`${project()?.name ?? "Project"} Dashboard`);
	});

	const project_slug = createMemo(() => params().project);
	const projectFetcher = createMemo(() => {
		return {
			project_slug: project_slug(),
			token: user?.token,
		};
	});
	const getProject = async (fetcher: {
		project_slug: string;
		token: string;
	}) => {
		const EMPTY_OBJECT = {};
		if (typeof fetcher.token !== "string") {
			return EMPTY_OBJECT;
		}
		if (props.project) {
			return props.project;
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

	const plotsFetcher = createMemo(() => {
		return {
			project_slug: project_slug(),
			token: user?.token,
		};
	});
	const getPlots = async (fetcher: {
		project_slug: string;
		token: string;
	}) => {
		const EMPTY_ARRAY: JsonPlot[] = [];
		if (typeof fetcher.token !== "string") {
			return EMPTY_ARRAY;
		}
		const path = `/v0/projects/${fetcher.project_slug}/plots?per_page=${MAX_PLOTS}`;
		return await httpGet(props.apiUrl, path, fetcher.token)
			.then((resp) => {
				return resp?.data as JsonPlot[];
			})
			.catch((error) => {
				console.error(error);
				return EMPTY_ARRAY;
			});
	};
	const [plots, { refetch }] = createResource<JsonPlot[]>(
		plotsFetcher,
		getPlots,
	);

	const allowedFetcher = createMemo(() => {
		return {
			apiUrl: props.apiUrl,
			params: props.params,
		};
	});
	const getAllowed = async (fetcher: {
		apiUrl: string;
		params: Params;
	}) => {
		return await isAllowedProjectManage(fetcher.apiUrl, fetcher.params);
	};
	const [isAllowed] = createResource(allowedFetcher, getAllowed);

	return (
		<div class="columns is-multiline is-vcentered">
			<For each={plots()}>
				{(plot, index) => (
					<div class="column is-11-tablet is-12-desktop is-6-widescreen">
						<Pinned
							apiUrl={props.apiUrl}
							params={props.params}
							user={user}
							project={project}
							isAllowed={isAllowed}
							plot={plot}
							index={index}
							refresh={refetch}
						/>
					</div>
				)}
			</For>
			<Show when={isAllowed() && plots()?.length <= MAX_PLOTS}>
				<div class="column is-11-tablet is-12-desktop is-6-widescreen">
					<PinNewPlot project_slug={project_slug} />
				</div>
			</Show>
		</div>
	);
};

const PinNewPlot = (props: { project_slug: Accessor<undefined | string> }) => {
	return (
		<div class="box">
			<div class="content has-text-centered">
				<h2 class="title is-2">Add a Perf Plot to your Project Dashboard</h2>
			</div>
			<div class="content">
				<ol>
					<li>Create a Perf Plot that you want to track.</li>
					<li>
						Click the <code>Pin</code> button.
					</li>
					<li>Name the pinned Perf Plot and set the time window.</li>
				</ol>
			</div>
			<a
				type="button"
				class="button is-primary is-fullwidth"
				href={`/console/projects/${props.project_slug()}/perf?clear=true`}
			>
				Create a Perf Plot
			</a>
		</div>
	);
};

export default DashboardPanel;
