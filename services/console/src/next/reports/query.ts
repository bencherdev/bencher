import type { QueryClient } from "@tanstack/solid-query";
import type { JsonReport } from "../../types/bencher";
import type { Api } from "../api";
import { NARROW } from "../plot/narrow";
import { batchSize } from "./rows";
import {
	type Batch,
	type ReportsSearch,
	apiParams,
	decodeSearch,
	encodeSearch,
} from "./search";

export interface ReportsBatch {
	reports: JsonReport[];
	/** Every report the search matches, from `X-Total-Count`. */
	total: number;
	batch: Batch;
}

/** The fixed height a row is drawn at, wide and narrow. */
export const ROW_HEIGHT = { wide: 40, narrow: 60 } as const;

/** The size of every batch on this screen: what fills it, plus a margin. */
export const screenBatch = () =>
	batchSize(
		window.innerHeight,
		window.matchMedia(NARROW).matches ? ROW_HEIGHT.narrow : ROW_HEIGHT.wide,
	);

export const batchKey = (slug: string, search: ReportsSearch, batch: Batch) =>
	[
		"console",
		"reports",
		slug,
		encodeSearch(search),
		batch.perPage,
		batch.page,
	] as const;

/**
 * One batch of a project's reports for a search, newest first. Each batch is
 * its own query, so a batch revalidates, fails, and retries on its own.
 */
export const batchQuery = (
	api: Api,
	slug: string,
	search: ReportsSearch,
	batch: Batch,
) => ({
	queryKey: batchKey(slug, search, batch),
	queryFn: async ({
		signal,
	}: {
		signal: AbortSignal;
	}): Promise<ReportsBatch> => {
		const params = apiParams(search, Date.now(), batch);
		const { data, headers } = await api.get<JsonReport[]>(
			`/v0/projects/${encodeURIComponent(slug)}/reports?${params}`,
			signal,
		);
		const total = Number(headers.get("X-Total-Count"));
		return {
			reports: data,
			total: Number.isFinite(total) ? total : data.length,
			batch,
		};
	},
	// A new search keeps the rows on screen, marked busy, until its first batch answers.
	placeholderData: (
		previous: ReportsBatch | undefined,
		query: { queryKey: readonly unknown[] } | undefined,
	) => (batch.page === 1 && query?.queryKey[2] === slug ? previous : undefined),
});

/** The name of a branch or a testbed, by slug. */
export const nameKey = (
	resource: "branches" | "testbeds",
	slug: string,
	value: string | undefined,
) => ["console", resource, slug, "name", value] as const;

/** Start the first batch for a Reports link before the page asks for it. */
export const prefetchReports = async (
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
