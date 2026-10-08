import type { QueryClient } from "@tanstack/solid-query";
import type { JsonConsoleReport } from "../../types/bencher";
import type { Api } from "../api";
import { NARROW } from "../plot/narrow";
import { batchSize } from "../reports/rows";
import { type ParameterSet, parameterEntries } from "./row";
import { type ReportView, reportViewSearch, windowDays } from "./view";

/** The height a row is drawn at, wide and narrow; a batch fills the screen with them. */
const ROW_HEIGHT = { wide: 44, narrow: 64 } as const;

/** The size of every batch on this screen, fixed for the page's life so batches line up. */
export const screenBatch = () =>
	batchSize(
		window.innerHeight,
		window.matchMedia(NARROW).matches ? ROW_HEIGHT.narrow : ROW_HEIGHT.wide,
	);

/** Whether the API could name a report this way; it refuses anything else before looking. */
export const isReportId = (id: string) => REPORT_ID.test(id);

/** A batch of a report's lines, and how they were grouped, which a held batch keeps. */
export interface ReportBatch {
	report: JsonConsoleReport;
	group: ReportView["group"];
}

/** One batch of a report's lines in drawing order, with the report's identity; each revalidates, fails, and retries on its own. */
export const batchQuery = (
	api: Api,
	slug: string,
	report: string,
	view: ReportView,
	batch: Batch,
) => ({
	queryKey: [
		"console",
		"report",
		slug,
		report,
		linesView(view),
		batch.perPage,
		batch.page,
	] as const,
	queryFn: async ({
		signal,
	}: {
		signal: AbortSignal;
	}): Promise<ReportBatch> => {
		const params = new URLSearchParams({
			window: String(windowDays(view.window)),
		});
		if (view.group !== "benchmark") {
			params.set("group", view.group);
		}
		if (view.sort !== "name") {
			params.set("sort", view.sort);
		}
		if (view.search) {
			params.set("search", view.search);
		}
		params.set("page", String(batch.page));
		params.set("per_page", String(batch.perPage));
		const { data } = await api.get<JsonConsoleReport>(
			`${reportPath(slug, report)}?${params}`,
			signal,
		);
		return { report: data, group: view.group };
	},
	// A new view keeps the rows on screen, marked busy, until its first batch answers.
	placeholderData: (
		previous: ReportBatch | undefined,
		query: { queryKey: readonly unknown[] } | undefined,
	) =>
		batch.page === 1 && query?.queryKey[3] === report ? previous : undefined,
});

/** The lines a threshold would check: one measure and metric name, and the variants carrying a set. */
export interface LineFilter {
	measure: string;
	metric: string;
	parameters?: ParameterSet | undefined;
}

/** How many of a report's lines match a filter, asking for none of them. */
export const countQuery = (
	api: Api,
	slug: string,
	report: string,
	filter: LineFilter,
) => {
	const params = new URLSearchParams({
		per_page: "0",
		measure: filter.measure,
		metric: filter.metric,
	});
	if (filter.parameters) {
		params.set(
			"parameters",
			JSON.stringify(Object.fromEntries(parameterEntries(filter.parameters))),
		);
	}
	return {
		queryKey: ["console", "report", slug, report, "count", `${params}`],
		queryFn: async ({ signal }: { signal: AbortSignal }) =>
			(
				await api.get<JsonConsoleReport>(
					`${reportPath(slug, report)}?${params}`,
					signal,
				)
			).data.total,
	};
};

/** Start a report's first batch, at the page's batch size, before the reader opens it. */
export const prefetchReport = (
	client: QueryClient,
	api: Api,
	slug: string,
	report: string,
	view: ReportView,
	perPage: number,
) =>
	client
		.query(batchQuery(api, slug, report, view, { page: 1, perPage }))
		.catch(() => {});

// A UUID, hyphenated or simple, as the API parses one.
const REPORT_ID =
	/^(?:[0-9a-f]{32}|[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12})$/i;

interface Batch {
	page: number;
	perPage: number;
}

const reportPath = (slug: string, report: string) =>
	`/v0/projects/${encodeURIComponent(slug)}/console/reports/${encodeURIComponent(report)}`;

/** The view's lines without the rows it has open, which change no request. */
const linesView = (view: ReportView) =>
	reportViewSearch({ ...view, expanded: [] });
