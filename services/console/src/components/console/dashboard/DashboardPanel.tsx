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
		<>
			<For each={plotPairs()}>
				{([left, right]) => (
					<div class="columns">
						<div class="column is-half">
							<div id={left?.uuid} class="box">
								<PinnedPlot plot={left} />
								<PinnedSettings plot={left} />
							</div>
						</div>
						<div class="column is-half">
							<Show
								when={right}
								fallback={<PinNewPlot project_slug={project_slug} />}
							>
								<div id={(right as JsonPlot)?.uuid} class="box">
									<PinnedPlot plot={right as JsonPlot} />
									<PinnedSettings plot={right as JsonPlot} />
								</div>
							</Show>
						</div>
					</div>
				)}
			</For>
			<Show when={plots()?.length % 2 === 0}>
				<div class="columns">
					<div class="column is-half">
						<PinNewPlot project_slug={project_slug} />
					</div>
				</div>
			</Show>
		</>
	);
};

const PinnedPlot = (props: { plot: JsonPlot }) => {
	const plotQueryString = () => {
		const newParams = new URLSearchParams();
		newParams.set(PlotKey.LowerValue, props.plot?.lower_value.toString());
		newParams.set(PlotKey.UpperValue, props.plot?.upper_value.toString());
		newParams.set(PlotKey.LowerBoundary, props.plot?.lower_boundary.toString());
		newParams.set(PlotKey.UpperBoundary, props.plot?.upper_boundary.toString());
		newParams.set(PlotKey.XAxis, props.plot?.x_axis);
		newParams.set(PerfQueryKey.Branches, props.plot?.branches.toString());
		newParams.set(PerfQueryKey.Testbeds, props.plot?.testbeds.toString());
		newParams.set(PerfQueryKey.Benchmarks, props.plot?.benchmarks.toString());
		newParams.set(PerfQueryKey.Measures, props.plot?.measures.toString());
		const now = new Date().getTime();
		newParams.set(
			PerfQueryKey.StartTime,
			(now - (props.plot?.window ?? 0) * 1_000).toString(),
		);
		newParams.set(PerfQueryKey.EndTime, now.toString());
		return newParams.toString();
	};

	return (
		<iframe
			loading="lazy"
			src={`/perf/${props.plot?.project}/embed?embed_logo=&embed_title=${
				props.plot?.name
			}&embed_header=false&embed_key=false&${plotQueryString()}`}
			title={props.plot?.name}
			width="100%"
			height="600px"
		/>
	);
};

const PinnedSettings = (props: { plot: JsonPlot }) => {
	return (
		<div class="buttons is-right">
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
