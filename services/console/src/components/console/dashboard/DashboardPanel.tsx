import type { Params } from "astro";
import {
	PerfQueryKey,
	PlotKey,
	type JsonPlot,
	type JsonProject,
} from "../../../types/bencher";
import {
	For,
	Show,
	createEffect,
	createMemo,
	createResource,
	type Accessor,
} from "solid-js";
import { setPageTitle } from "../../../util/resource";
import { authUser } from "../../../util/auth";
import { httpGet } from "../../../util/http";
import { plotQueryString } from "./util";

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
		const path = `/v0/projects/${fetcher.project_slug}/plots`;
		return await httpGet(props.apiUrl, path, fetcher.token)
			.then((resp) => {
				return resp?.data as JsonPlot[];
			})
			.catch((error) => {
				console.error(error);
				return EMPTY_ARRAY;
			});
	};
	const [plots] = createResource<JsonPlot[]>(plotsFetcher, getPlots);

	const plotPairs = createMemo(() => {
		const p = plots();
		const pairs: [JsonPlot, undefined | JsonPlot][] = [];
		if (!p) {
			return pairs;
		}
		for (let i = 0; i < p.length; i += 2) {
			const left = p[i] as JsonPlot;
			const right = p[i + 1];
			pairs.push([left, right]);
		}
		return pairs;
	});

	return (
		<div class="columns is-multiline is-vcentered">
			<For each={plots()}>
				{(plot) => (
					<div class="column is-11-tablet is-12-desktop is-6-widescreen">
						<div id={plot?.uuid} class="box">
							<PinnedPlot plot={plot} />
							<PinnedSettings plot={plot} />
						</div>
					</div>
				)}
			</For>
			<div class="column is-11-tablet is-12-desktop is-6-widescreen">
				<PinNewPlot project_slug={project_slug} />
			</div>
		</div>
	);
};

const PinnedPlot = (props: { plot: JsonPlot }) => {
	return (
		<iframe
			loading="lazy"
			src={`/perf/${props.plot?.project}/embed?embed_logo=false&embed_title=${
				props.plot?.name
			}&embed_header=false&embed_key=false&${plotQueryString(props.plot)}`}
			title={props.plot?.name}
			width="100%"
			height="600px"
		/>
	);
};

const PinnedSettings = (props: { plot: JsonPlot }) => {
	return (
		<div class="buttons is-right">
			<a
				type="button"
				class="button is-small"
				title={`View ${props.plot?.name} Perf Plot`}
				href={`/console/projects/${props.plot?.project}/perf?${plotQueryString(
					props.plot,
				)}`}
			>
				<span class="icon is-small">
					<i class="fas fa-external-link-alt" />
				</span>
			</a>
			<button type="button" class="button is-small">
				<span class="icon is-small">
					<i class="fas fa-cog" />
				</span>
			</button>
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
