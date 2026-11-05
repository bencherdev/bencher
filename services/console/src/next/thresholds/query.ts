import type {
	JsonConsoleAlerts,
	JsonConsoleThreshold,
	JsonConsoleThresholds,
} from "../../types/bencher";
import type { Batch as AlertsBatch } from "../alerts/search";
import type { Api } from "../api";
import { NARROW } from "../plot/narrow";
import { batchSize } from "../reports/rows";
import {
	type Batch,
	type ThresholdView,
	type ThresholdsSearch,
	alertsParams,
	encodeSearch,
	encodeView,
	thresholdsParams,
} from "./search";

/** The fixed height a list row is drawn at, by the lines it takes. */
export const ROW_HEIGHT = { 1: 40, 2: 60, 3: 78, 4: 96, 5: 114 } as const;
/** The alerts endpoint pages at most this many. */
const MAX_ALERTS = 64;
/** The fixed height an alert's row is drawn at, wide and narrow, as a report's line rows are. */
const ALERT_HEIGHT = { wide: 44, narrow: 64 } as const;

/** The size of every batch of the list on this screen: what fills it, plus a margin. */
export const screenBatch = () =>
	batchSize(window.innerHeight, narrow() ? ROW_HEIGHT[2] : ROW_HEIGHT[1]);

const narrow = () => window.matchMedia(NARROW).matches;

/** The size of every batch of a threshold's alerts on this screen. */
export const alertsBatch = () =>
	Math.min(
		MAX_ALERTS,
		batchSize(
			window.innerHeight,
			narrow() ? ALERT_HEIGHT.narrow : ALERT_HEIGHT.wide,
		),
	);

/**
 * One batch of a project's thresholds, oldest first, with the alerts each
 * raised in the window. Each batch is its own query, so it revalidates, fails,
 * and retries on its own.
 */
export const thresholdsQuery = (
	api: Api,
	slug: string,
	search: ThresholdsSearch,
	batch: Batch,
) => ({
	queryKey: [
		"console",
		"thresholds",
		slug,
		encodeSearch(search),
		batch.perPage,
		batch.page,
	] as const,
	queryFn: async ({ signal }: { signal: AbortSignal }) =>
		(
			await api.get<JsonConsoleThresholds>(
				`${consolePath(slug)}/thresholds?${thresholdsParams(search, Date.now(), batch)}`,
				signal,
			)
		).data,
	// A new search keeps the rows on screen, marked busy, until its first batch answers.
	placeholderData: (
		previous: JsonConsoleThresholds | undefined,
		query: { queryKey: readonly unknown[] } | undefined,
	) => (batch.page === 1 && query?.queryKey[2] === slug ? previous : undefined),
});

const consolePath = (slug: string) =>
	`/v0/console/projects/${encodeURIComponent(slug)}`;

/** A threshold with its model history. */
export const thresholdQuery = (api: Api, slug: string, threshold: string) => ({
	queryKey: ["console", "threshold", slug, threshold] as const,
	queryFn: async ({ signal }: { signal: AbortSignal }) =>
		(
			await api.get<JsonConsoleThreshold>(
				`${consolePath(slug)}/thresholds/${encodeURIComponent(threshold)}`,
				signal,
			)
		).data,
});

/** One batch of the alerts a threshold raised, newest report first, each with its history. */
export const alertsQuery = (
	api: Api,
	slug: string,
	threshold: string,
	view: ThresholdView,
	batch: AlertsBatch & { round: number },
) => ({
	queryKey: [
		...alertsViewKey(slug, threshold, view),
		batch.perPage,
		batch.offset,
		batch.round,
	] as const,
	queryFn: async ({ signal }: { signal: AbortSignal }) =>
		(
			await api.get<JsonConsoleAlerts>(
				`${consolePath(slug)}/alerts?${alertsParams(threshold, view, Date.now(), batch)}`,
				signal,
			)
		).data,
	// A new view keeps the rows on screen, marked busy, until its first batch answers.
	placeholderData: (
		previous: JsonConsoleAlerts | undefined,
		query: { queryKey: readonly unknown[] } | undefined,
	) =>
		batch.offset === 0 && batch.round === 0 && query?.queryKey[3] === threshold
			? previous
			: undefined,
});

/** Every batch of one view of the alerts a threshold raised. */
const alertsViewKey = (slug: string, threshold: string, view: ThresholdView) =>
	[
		"console",
		"threshold",
		slug,
		threshold,
		"alerts",
		encodeView(view),
	] as const;
