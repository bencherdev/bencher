import bencher_valid_init, { type InitOutput } from "bencher_valid";
import type { Params } from "astro";
import type { JsonPlot, JsonProject } from "../../../types/bencher";
import {
	For,
	Show,
	createEffect,
	createMemo,
	createResource,
	createSignal,
	type Accessor,
} from "solid-js";
import { setPageTitle } from "../../../util/resource";
import { authUser, isAllowedProjectManage } from "../../../util/auth";
import { httpGet } from "../../../util/http";
import Pinned from "./Pinned";
import { validJwt } from "../../../util/valid";
import { createStore } from "solid-js/store";

const MAX_PLOTS = 255;

export interface Props {
	apiUrl: string;
	params: Params;
	project?: undefined | JsonProject;
}

const PlotsPanel = (props: Props) => {
	const [bencher_valid] = createResource(
		async () => await bencher_valid_init(),
	);

	const params = createMemo(() => props.params);
	const user = authUser();

	createEffect(() => {
		setPageTitle(`${project()?.name ?? "Project"} Plots`);
	});

	const project_slug = createMemo(() => params().project);
	const projectFetcher = createMemo(() => {
		return {
			bencher_valid: bencher_valid(),
			project_slug: project_slug(),
			token: user?.token,
		};
	});
	const getProject = async (fetcher: {
		bencher_valid: InitOutput;
		project_slug: string;
		token: string;
	}) => {
		const EMPTY_OBJECT = {};
		if (!fetcher.bencher_valid) {
			return EMPTY_OBJECT;
		}
		if (props.project) {
			return props.project;
		}
		if (!validJwt(fetcher.token)) {
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

	const plotsFetcher = createMemo(() => {
		return {
			bencher_valid: bencher_valid(),
			project_slug: project_slug(),
			token: user?.token,
		};
	});
	const getPlots = async (fetcher: {
		bencher_valid: InitOutput;
		project_slug: string;
		token: string;
	}) => {
		const EMPTY_ARRAY: JsonPlot[] = [];
		if (!fetcher.bencher_valid) {
			return EMPTY_ARRAY;
		}
		if (!validJwt(fetcher.token)) {
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
	const [plots] = createResource<JsonPlot[]>(plotsFetcher, getPlots);
	const [plotArray, setPlotArray] = createStore<JsonPlot[]>([]);
	// This is necessary to keep the fetched plots from overwriting the plots store after a deletion.
	const [plotsLoaded, setPlotsLoaded] = createSignal(false);
	const plotsLength = createMemo(() => plots()?.length);

	createEffect(() => {
		// It is important to check the array length for the fetched plots,
		// as this is a way to make sure we only set the plots store once the fetched plots are actually loaded.
		if (plots() && plotArray.length !== plotsLength() && !plotsLoaded()) {
			setPlotArray(plots());
			setPlotsLoaded(true);
		}
	});

	const movePlot = (from: number, to: number) => {
		console.log(from, to);
		const newPlots = [...plotArray];
		const [removed] = newPlots.splice(from, 1);
		newPlots.splice(to, 0, removed);
		setPlotArray(newPlots);
	};

	const updatePlot = (index: number, plot: JsonPlot) => {
		const newPlots = [...plotArray];
		newPlots[index] = plot;
		setPlotArray(newPlots);
	};

	const removePlot = (index: number) => {
		const newPlots = [...plotArray];
		newPlots.splice(index, 1);
		setPlotArray(newPlots);
	};

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
			<For each={plotArray}>
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
							total={plotsLength}
							movePlot={movePlot}
							updatePlot={updatePlot}
							removePlot={removePlot}
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
				<h2 class="title is-2">Pin a Perf Plot</h2>
			</div>
			<div class="content">
				<ol>
					<li>Create a Perf Plot that you want to track.</li>
					<li>
						Click the <code>Pin</code> button.
					</li>
					<li>Name the Perf Plot and set a time window.</li>
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

export default PlotsPanel;
