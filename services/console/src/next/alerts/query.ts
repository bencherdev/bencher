import type {
	JsonConsoleAlerts,
	JsonUpdateAlerts,
	JsonUpdatedAlerts,
} from "../../types/bencher";
import type { Api } from "../api";
import { batchSize } from "../reports/rows";
import { FOLD } from "./fold";
import {
	type AlertsSearch,
	type Batch,
	type Bounds,
	alertsParams,
	encodeSearch,
	windowBounds,
} from "./search";

/** The alerts endpoint pages at most this many. */
const MAX_ALERTS = 64;
/** The height an alert's row is drawn at, wide and narrow, as a report's line rows are. */
const ROW_HEIGHT = { wide: 44, narrow: 64 } as const;

/** The size of every batch on this screen: what fills it, plus a margin. */
export const alertsBatch = () =>
	Math.min(
		MAX_ALERTS,
		batchSize(
			window.innerHeight,
			window.matchMedia(FOLD).matches ? ROW_HEIGHT.narrow : ROW_HEIGHT.wide,
		),
	);

/** One batch of the list, and the window it was read over, which Dismiss all binds again. */
export interface AlertsBatch {
	alerts: JsonConsoleAlerts;
	bounds: Bounds;
}

const alertsPath = (slug: string) =>
	`/v0/projects/${encodeURIComponent(slug)}/console/alerts`;

/** Every batch of one search. */
export const searchKey = (slug: string, search: AlertsSearch) =>
	["console", "alerts", slug, encodeSearch(search)] as const;

/**
 * One batch of a project's alerts, newest report first. Each batch is its own
 * query, so it revalidates, fails, and retries on its own; a rolling window is
 * measured back from `now`.
 */
export const alertsQuery = (
	api: Api,
	slug: string,
	search: AlertsSearch,
	batch: Batch & { round: number },
	now: number,
) => ({
	queryKey: [
		...searchKey(slug, search),
		batch.perPage,
		batch.offset,
		batch.round,
	] as const,
	queryFn: async ({
		signal,
	}: {
		signal: AbortSignal;
	}): Promise<AlertsBatch> => {
		const bounds = windowBounds(search.window, now);
		const { data } = await api.get<JsonConsoleAlerts>(
			`${alertsPath(slug)}?${alertsParams(search, bounds, batch)}`,
			signal,
		);
		return { alerts: data, bounds };
	},
	// A new search keeps the rows on screen, marked busy, until its first batch answers.
	placeholderData: (
		previous: AlertsBatch | undefined,
		query: { queryKey: readonly unknown[] } | undefined,
	) =>
		batch.offset === 0 && batch.round === 0 && query?.queryKey[2] === slug
			? previous
			: undefined,
});

/** Change the alerts `body` selects; resolves to how many changed. */
export const updateAlerts = async (
	api: Api,
	slug: string,
	body: JsonUpdateAlerts,
) =>
	(await api.send<JsonUpdatedAlerts>("PATCH", alertsPath(slug), body)).data
		.changed;

export type Dimension = "branches" | "testbeds" | "measures";

/** The name of a branch, a testbed, or a measure, by UUID, as the Thresholds pages key it. */
export const nameKey = (resource: Dimension, slug: string, uuid: string) =>
	["console", resource, slug, "name", uuid] as const;

export const nameQuery = (
	api: Api,
	slug: string,
	resource: Dimension,
	uuid: string | undefined,
) => ({
	queryKey: nameKey(resource, slug, uuid ?? ""),
	queryFn: async ({ signal }: { signal: AbortSignal }) =>
		(
			await api.get<{ name: string }>(
				`/v0/projects/${encodeURIComponent(slug)}/${resource}/${encodeURIComponent(uuid ?? "")}`,
				signal,
			)
		).data.name,
	enabled: uuid !== undefined,
});
