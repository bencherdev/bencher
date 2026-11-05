import type { QueryClient } from "@tanstack/solid-query";
import type {
	JsonConsoleRunSummary,
	JsonConsoleRuns,
} from "../../types/bencher";
import type { Api } from "../api";
import {
	type Batch,
	type ReportsSearch,
	apiParams,
	decodeSearch,
	encodeSearch,
} from "../reports/search";
import { screenBatch } from "../list/rows";
import { live, unfinished } from "./status";

export interface RunsBatch {
	runs: JsonConsoleRunSummary[];
	/** Every run the search matches. */
	total: number;
	batch: Batch;
}

export const batchKey = (slug: string, search: ReportsSearch, batch: Batch) =>
	[
		"console",
		"runs",
		slug,
		encodeSearch(search),
		batch.perPage,
		batch.page,
	] as const;

/**
 * One batch of a project's runs for a search, newest first. Each batch is its
 * own query, so a batch revalidates, fails, and retries on its own.
 */
export const batchQuery = (
	api: Api,
	slug: string,
	search: ReportsSearch,
	batch: Batch,
) => ({
	queryKey: batchKey(slug, search, batch),
	queryFn: async ({ signal }: { signal: AbortSignal }): Promise<RunsBatch> => {
		const params = apiParams(search, Date.now(), batch);
		const { data } = await api.get<JsonConsoleRuns>(
			`/v0/console/projects/${encodeURIComponent(slug)}/runs?${params}`,
			signal,
		);
		return { runs: data.runs, total: data.total, batch };
	},
	// A new search keeps the rows on screen, marked busy, until its first batch answers.
	placeholderData: (
		previous: RunsBatch | undefined,
		query: { queryKey: readonly unknown[] } | undefined,
	) => (batch.page === 1 && query?.queryKey[2] === slug ? previous : undefined),
	...live((batch: RunsBatch) => batch.runs.some((run) => unfinished(run.jobs))),
});

/** Start the first batch for a Runs link before the page asks for it. */
export const prefetchRuns = async (
	client: QueryClient,
	api: Api,
	slug: string,
	query: string,
) => {
	const search = decodeSearch(new URLSearchParams(query));
	await client
		.query(batchQuery(api, slug, search, { page: 1, perPage: screenBatch() }))
		.catch(() => {});
};
