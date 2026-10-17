import type { JsonConsoleRun } from "../../types/bencher";
import type { Api } from "../api";
import type { Batch } from "../reports/search";
import { live, unfinished } from "./status";

/** Whether the API could name a run this way; it refuses anything else before looking. */
export const isRunId = (id: string) => RUN_ID.test(id);

/** One batch of a run's reports in start order, with the run's facts; each revalidates, fails, and retries on its own. */
export const runBatchQuery = (
	api: Api,
	slug: string,
	version: string,
	batch: Batch,
) => ({
	queryKey: [
		"console",
		"run",
		slug,
		version,
		batch.perPage,
		batch.page,
	] as const,
	queryFn: async ({ signal }: { signal: AbortSignal }) => {
		const params = new URLSearchParams({
			page: String(batch.page),
			per_page: String(batch.perPage),
		});
		const { data } = await api.get<JsonConsoleRun>(
			`/v0/console/projects/${encodeURIComponent(slug)}/runs/${encodeURIComponent(version)}?${params}`,
			signal,
		);
		return data;
	},
	// Each ask reads the whole run, so a later batch asks again only for its own reports.
	...live((run: JsonConsoleRun) =>
		batch.page === 1
			? unfinished(run.jobs)
			: run.reports.some((report) => unfinished(report.job)),
	),
});

const RUN_ID =
	/^(?:[0-9a-f]{32}|[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12})$/i;
