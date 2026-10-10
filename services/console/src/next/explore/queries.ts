import type {
	JsonAlert,
	JsonConsoleLatestReport,
	JsonConsolePerf,
	JsonPlot,
	JsonReport,
	JsonVariant,
} from "../../types/bencher";
import type { Api } from "../api";
import type { ExploreQuery, Parameters } from "../query/query";
import { perfSearch, requestKey } from "./request";

const project = (slug: string) => `/v0/projects/${encodeURIComponent(slug)}`;

/**
 * The plot query; `now` is read as it is asked, so a rolling window ends then.
 * A request an edit outruns finishes into the cache rather than being aborted,
 * so Back to that query draws at once; every read here does the same.
 */
export const perfQuery = (api: Api, slug: string, query: ExploreQuery) => ({
	queryKey: ["console", "perf", slug, requestKey(query)],
	queryFn: async () =>
		(
			await api.get<JsonConsolePerf>(
				`${project(slug)}/console/perf${perfSearch(query, Date.now())}`,
			)
		).data,
});

export const plotKey = (slug: string, plot: string) =>
	["console", "plot", slug, plot] as const;

export const plotQuery = (api: Api, slug: string, plot: string) => ({
	queryKey: plotKey(slug, plot),
	queryFn: async () =>
		(
			await api.get<JsonPlot>(
				`${project(slug)}/plots/${encodeURIComponent(plot)}`,
			)
		).data,
});

// The API's largest page: suggestions read every variant a benchmark has.
const ALL = 255;

/** A benchmark's variants, which suggest the parameters box's tags and name a pin. */
export const variantsQuery = (api: Api, slug: string, benchmark: string) => ({
	queryKey: ["console", "variants", slug, benchmark],
	queryFn: async (): Promise<Parameters[]> =>
		(
			await api.get<JsonVariant[]>(
				`${project(slug)}/benchmarks/${encodeURIComponent(benchmark)}/variants?per_page=${ALL}`,
			)
		).data.map(({ parameters }) => parameters),
	staleTime: 5 * 60_000,
});

export const latestQuery = (api: Api, slug: string, benchmark: string) => ({
	queryKey: ["console", "latest", slug, benchmark],
	queryFn: async () =>
		(
			await api.get<JsonConsoleLatestReport>(
				`${project(slug)}/console/benchmarks/${encodeURIComponent(benchmark)}/latest`,
			)
		).data,
});

export type Dimension = "branches" | "testbeds" | "benchmarks" | "measures";

interface Named {
	uuid: string;
	name: string;
	units?: string;
}

/** One value's name, for a value the plot drew no line of; a pick seeds it. */
export const nameQuery = (
	api: Api,
	slug: string,
	dimension: Dimension,
	uuid: string,
) => ({
	queryKey: ["console", "name", slug, dimension, uuid],
	queryFn: async () => {
		const { name, units } = (
			await api.get<Named>(
				`${project(slug)}/${dimension}/${encodeURIComponent(uuid)}`,
			)
		).data;
		return { name, ...(units === undefined ? {} : { units }) };
	},
});

/** How many values an add control lists at once; its search finds the rest. */
const OPTIONS = 32;

/** A project's branches, testbeds, benchmarks, or measures matching `text`. */
export const searchQuery = (
	api: Api,
	slug: string,
	dimension: Dimension,
	text: string,
) => ({
	queryKey: ["console", "search", slug, dimension, text],
	// The last options stay while the next search loads, so the list never blinks empty.
	placeholderData: (previous: Named[] | undefined) => previous,
	queryFn: async () => {
		const params = new URLSearchParams({ per_page: String(OPTIONS) });
		if (text) {
			params.set("search", text);
		}
		return (
			await api.get<Named[]>(`${project(slug)}/${dimension}?${params}`)
		).data.map(({ uuid, name, units }) => ({
			uuid,
			name,
			...(units === undefined ? {} : { units }),
		}));
	},
});

/** How many of each list blank Explore starts from. */
export const START = { alerts: 3, reports: 4, plots: 3 } as const;

const listQuery = <T>(api: Api, slug: string, name: string, path: string) => ({
	queryKey: ["console", "start", slug, name],
	queryFn: async () => (await api.get<T[]>(`${project(slug)}${path}`)).data,
});

export const startQueries = (api: Api, slug: string) => ({
	alerts: listQuery<JsonAlert>(
		api,
		slug,
		"alerts",
		`/alerts?status=active&sort=created&direction=desc&per_page=${START.alerts}`,
	),
	reports: listQuery<JsonReport>(
		api,
		slug,
		"reports",
		`/reports?per_page=${START.reports}`,
	),
	plots: listQuery<JsonPlot>(
		api,
		slug,
		"plots",
		`/plots?per_page=${START.plots}`,
	),
});
