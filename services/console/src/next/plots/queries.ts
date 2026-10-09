import type { JsonConsolePlots } from "../../types/bencher";
import type { Api } from "../api";
import { PRESETS } from "../explore/window";
import { NARROW } from "../plot/narrow";

/** None leaves each plot at its own saved window. */
export type PageWindow =
	| (typeof PRESETS)[number]["value"]
	| { start: number; end: number }
	| undefined;

export interface PlotsBatch {
	page: number;
	perPage: number;
}

/** The API's search for a page window; a preset, and each plot's own, ends now. */
export const pageWindowSearch = (
	window: PageWindow,
	now: number,
): Record<string, string> => {
	if (window === undefined) {
		return { end_time: String(now) };
	}
	if (typeof window === "object") {
		return { start_time: String(window.start), end_time: String(window.end) };
	}
	const seconds = PRESETS.find(({ value }) => value === window)?.seconds ?? 0;
	return { start_time: String(now - seconds * 1000), end_time: String(now) };
};

export const plotsKey = (slug: string, window: PageWindow, batch: PlotsBatch) =>
	[
		"console",
		"plots",
		slug,
		typeof window === "object" ? [window.start, window.end] : (window ?? null),
		batch.perPage,
		batch.page,
	] as const;

/** Each batch is its own query, so it revalidates, fails, and retries on its own. */
export const plotsQuery = (
	api: Api,
	slug: string,
	window: PageWindow,
	batch: PlotsBatch,
) => ({
	queryKey: plotsKey(slug, window, batch),
	queryFn: async () => {
		const search = new URLSearchParams({
			page: String(batch.page),
			per_page: String(batch.perPage),
			...pageWindowSearch(window, Date.now()),
		});
		return (
			await api.get<JsonConsolePlots>(
				`/v0/console/projects/${encodeURIComponent(slug)}/plots?${search}`,
			)
		).data;
	},
});

/** A tile's height with the grid's gap, wide and narrow, as `plots.css` draws it. */
const TILE = { wide: 437, narrow: 548 } as const;
/** The grid's narrowest column and its gap, and the page's widest content and its padding. */
const GRID = { column: 540, gap: 12, page: 1440, padding: 48 } as const;

/** The tiles a batch holds on a screen this size: every column, rows to fill it and half again. */
export const tileBatch = (width: number, height: number, narrow: boolean) => {
	const content = Math.min(width, GRID.page) - GRID.padding;
	const columns = narrow
		? 1
		: Math.max(1, Math.floor((content + GRID.gap) / (GRID.column + GRID.gap)));
	const rows = Math.ceil(height / (narrow ? TILE.narrow : TILE.wide));
	return columns * (rows + Math.ceil(rows / 2));
};

export const screenTiles = () =>
	tileBatch(
		window.innerWidth,
		window.innerHeight,
		window.matchMedia(NARROW).matches,
	);
