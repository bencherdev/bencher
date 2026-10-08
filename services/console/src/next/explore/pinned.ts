import {
	type JsonConsolePerf,
	type JsonConsolePerfLine,
	type JsonNewPlot,
	type JsonPlot,
	PlotLayout,
	XAxis,
	YAxis,
} from "../../types/bencher";
import type { Api } from "../api";
import { lineVisible } from "../query/line";
import {
	type ExploreQuery,
	type Parameters,
	blankQuery,
	encodeQuery,
} from "../query/query";
import { type PinTitleLine, defaultPinTitle, pinWindow } from "./pin";

type Scale = ExploreQuery["yScale"];

const SCALES: Record<YAxis, Scale> = {
	[YAxis.Auto]: "auto",
	[YAxis.Linear]: "linear",
	[YAxis.Log]: "log",
};
const Y_AXES: Record<Scale, YAxis> = {
	auto: YAxis.Auto,
	linear: YAxis.Linear,
	log: YAxis.Log,
};

/** The query a pinned plot saved, editing that pin. */
export const queryFromPlot = (plot: JsonPlot): ExploreQuery => ({
	...blankQuery(),
	branches: plot.branches.map((uuid) => ({ uuid })),
	testbeds: plot.testbeds.map((uuid) => ({ uuid })),
	benchmarks: plot.benchmarks,
	sets: plot.parameters ?? [],
	measures: plot.measures,
	metrics: plot.metrics ?? [],
	xAxis: plot.x_axis === XAxis.Version ? "version" : "date",
	yScale: SCALES[plot.y_axis],
	window: { seconds: plot.window },
	layout: plot.layout === PlotLayout.Stacked ? "stacked" : "dual",
	hide: plot.hidden ?? [],
	...(plot.focus === undefined ? {} : { focus: plot.focus }),
	plot: plot.uuid,
});

/** Explore's search that opens a pinned plot. */
export const plotSearch = (plot: JsonPlot): string =>
	encodeQuery(queryFromPlot(plot));

/** A new pin of the query, at the top of Plots; `drawn` are the keys of the lines drawn. */
export const newPlot = (
	query: ExploreQuery,
	title: string,
	drawn: readonly string[],
	now: number,
): JsonNewPlot => ({
	title,
	// The classic perf page's own defaults; the new console draws from the query alone.
	lower_value: false,
	upper_value: false,
	lower_boundary: false,
	upper_boundary: false,
	...view(query, drawn, now),
	...(query.focus === undefined ? {} : { focus: query.focus }),
});

/** What a pin saves of the query; pins keep no heads or specs, so they follow their branches. */
const view = (query: ExploreQuery, drawn: readonly string[], now: number) => ({
	x_axis: query.xAxis === "version" ? XAxis.Version : XAxis.DateTime,
	y_axis: Y_AXES[query.yScale],
	layout: query.layout === "stacked" ? PlotLayout.Stacked : PlotLayout.Dual,
	window: pinWindow(query.window, now),
	branches: query.branches.map(({ uuid }) => uuid),
	testbeds: query.testbeds.map(({ uuid }) => uuid),
	benchmarks: [...query.benchmarks],
	parameters: [...query.sets],
	measures: [...query.measures],
	metrics: [...query.metrics],
	hidden: hiddenAmong(query, drawn),
});

/** The keys of the drawn lines the query hides, which is what a pin keeps. */
const hiddenAmong = (query: ExploreQuery, drawn: readonly string[]) =>
	drawn.filter((key) => !lineVisible(query, key));

/** Pin a plot; the report page's Pin N makes one per line with `newPlot`. */
export const pinPlot = async (api: Api, slug: string, plot: JsonNewPlot) =>
	(await api.send<JsonPlot>("POST", plotsPath(slug), plot)).data;

export const unpinPlot = (api: Api, slug: string, plot: string) =>
	api.send("DELETE", `${plotsPath(slug)}/${encodeURIComponent(plot)}`);

const plotsPath = (slug: string) =>
	`/v0/projects/${encodeURIComponent(slug)}/plots`;

/** The query saved over a pin: an empty box or no focus clears what the pin held. */
export const plotPatch = (
	query: ExploreQuery,
	drawn: readonly string[],
	now: number,
) => ({ ...view(query, drawn, now), focus: query.focus ?? null });

/** Whether the query holds changes the pin has not saved. */
export const unsaved = (
	query: ExploreQuery,
	plot: JsonPlot,
	drawn: readonly string[],
): boolean => saved(query, drawn) !== saved(queryFromPlot(plot), drawn);

const saved = (query: ExploreQuery, drawn: readonly string[]) =>
	encodeQuery({
		...blankQuery(),
		branches: query.branches.map(({ uuid }) => ({ uuid })),
		testbeds: query.testbeds.map(({ uuid }) => ({ uuid })),
		benchmarks: query.benchmarks,
		sets: query.sets,
		measures: query.measures,
		metrics: query.metrics,
		xAxis: query.xAxis,
		yScale: query.yScale,
		window: query.window,
		layout: query.layout,
		hide: hiddenAmong(query, drawn).sort(),
		...(query.focus === undefined ? {} : { focus: query.focus }),
	});

/** Whether the answer's line at `index` shows; `drawn` holds the drawn lines' keys in the answer's order. */
export const shownAmong =
	(query: ExploreQuery, drawn: readonly string[]) =>
	(_: JsonConsolePerfLine, index: number): boolean => {
		const id = drawn[index];
		return id !== undefined && lineVisible(query, id);
	};

/** A pin's default title, naming the shown lines; `variants` holds each benchmark's variants by UUID. */
export const pinTitle = (
	perf: JsonConsolePerf,
	shown: (line: JsonConsolePerfLine, index: number) => boolean,
	variants: ReadonlyMap<string, readonly Parameters[]>,
): string => {
	const lines = perf.lines.filter(shown);
	const metrics = new Set(lines.map(({ metric }) => metric)).size > 1;
	return defaultPinTitle(
		lines.flatMap((line): PinTitleLine[] => {
			const benchmark = perf.benchmarks[line.benchmark];
			const variant = perf.variants[line.variant];
			const measure = perf.measures[line.measure];
			const branch = perf.branches[line.branch];
			if (!(benchmark && variant && measure && branch)) {
				return [];
			}
			return [
				{
					benchmark: benchmark.name,
					parameters: variant.parameters,
					variants:
						variants.get(benchmark.uuid) ??
						perf.variants
							.filter((other) => other.benchmark === line.benchmark)
							.map(({ parameters }) => parameters),
					measure: measure.name,
					...(metrics ? { metric: line.metric } : {}),
					branch: branch.name,
				},
			];
		}),
	);
};
