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
import { DEBOUNCE_DELAY, validJwt } from "../../../util/valid";
import { createStore } from "solid-js/store";
import { useSearchParams } from "../../../util/url";
import PlotsHeader from "./PlotsHeader";
import { debounce } from "@solid-primitives/scheduled";
import FallbackPlots from "./FallbackPlots";

const SEARCH_PARAM = "search";
const MAX_PLOTS = 64;

export interface Props {
	apiUrl: string;
	params: Params;
}

const PlotsPanel = (props: Props) => {
	const [bencher_valid] = createResource(
		async () => await bencher_valid_init(),
	);
	const [searchParams, setSearchParams] = useSearchParams();

	const params = createMemo(() => props.params);
	const user = authUser();

	createEffect(() => {
		setPageTitle(`${project()?.name ?? "Project"} Plots`);

		const initParams: Record<string, null | number | boolean> = {};
		if (typeof searchParams[SEARCH_PARAM] !== "string") {
			initParams[SEARCH_PARAM] = null;
		}
		if (Object.keys(initParams).length !== 0) {
			setSearchParams(initParams, { replace: true });
		}
	});

	const search = createMemo(() => searchParams[SEARCH_PARAM]);
	const handleSearch = debounce(
		(search: string) =>
			setSearchParams({ [SEARCH_PARAM]: search }, { scroll: true }),
		DEBOUNCE_DELAY,
	);

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

	const searchQuery = createMemo(() => {
		return {
			per_page: MAX_PLOTS,
			search: search(),
		};
	});
	const plotsFetcher = createMemo(() => {
		return {
			bencher_valid: bencher_valid(),
			project_slug: project_slug(),
			searchQuery: searchQuery(),
			token: user?.token,
		};
	});
	const getPlots = async (fetcher: {
		bencher_valid: InitOutput;
		project_slug: string;
		searchQuery: {
			per_page: number;
			search: undefined | string;
		};
		token: string;
	}) => {
		const EMPTY_ARRAY: JsonPlot[] = [];
		if (!fetcher.bencher_valid) {
			return EMPTY_ARRAY;
		}
		if (!validJwt(fetcher.token)) {
			return EMPTY_ARRAY;
		}
		const searchParams = new URLSearchParams();
		for (const [key, value] of Object.entries(fetcher.searchQuery)) {
			if (value) {
				searchParams.set(key, value.toString());
			}
		}
		const path = `/v0/projects/${
			fetcher.project_slug
		}/plots?${searchParams.toString()}`;
		return await httpGet(props.apiUrl, path, fetcher.token)
			.then((resp) => {
				setPlots(resp?.data as JsonPlot[]);
				return resp?.data as JsonPlot[];
			})
			.catch((error) => {
				console.error(error);
				return EMPTY_ARRAY;
			});
	};
	const [projectPlots, { refetch }] = createResource<JsonPlot[]>(
		plotsFetcher,
		getPlots,
	);
	const [plots, setPlots] = createStore<JsonPlot[]>([]);
	const plotsLength = createMemo(() => plots?.length);

	const movePlot = (from: number, to: number) => {
		const newPlots = [...plots];
		const [removed] = newPlots.splice(from, 1);
		newPlots.splice(to, 0, removed);
		setPlots(newPlots);
	};

	const updatePlot = (index: number, plot: JsonPlot) => {
		const newPlots = [...plots];
		newPlots[index] = plot;
		setPlots(newPlots);
	};

	const removePlot = (index: number) => {
		const newPlots = [...plots];
		newPlots.splice(index, 1);
		setPlots(newPlots);
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
		<>
			<PlotsHeader
				apiUrl={props.apiUrl}
				params={props.params}
				project={project}
				search={search}
				handleRefresh={refetch}
				handleSearch={handleSearch}
			/>
			<Show when={projectPlots.loading}>
				<FallbackPlots />
			</Show>
			<div class="columns is-multiline is-vcentered">
				<For each={plots}>
					{(plot, index) => (
						<div class="column is-11-tablet is-12-desktop is-6-widescreen">
							<Pinned
								apiUrl={props.apiUrl}
								params={props.params}
								project_slug={project_slug}
								user={user}
								isAllowed={isAllowed}
								plot={plot}
								index={index}
								total={plotsLength}
								movePlot={movePlot}
								updatePlot={updatePlot}
								removePlot={removePlot}
								search={search}
							/>
						</div>
					)}
				</For>
				<Show when={isAllowed() && plotsLength() < MAX_PLOTS}>
					<div class="column is-11-tablet is-12-desktop is-6-widescreen">
						<PinNewPlot project_slug={project_slug} />
					</div>
				</Show>
			</div>
		</>
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
