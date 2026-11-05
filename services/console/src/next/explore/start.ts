import type {
	JsonAlert,
	JsonConsoleBranch,
	JsonConsoleMeasure,
	JsonConsoleReport,
	JsonConsoleRunReport,
	JsonConsoleRunSummary,
	JsonConsoleRuns,
	JsonPlot,
} from "../../types/bencher";
import type { Api } from "../api";
import { projectPath } from "../paths";
import {
	DEFAULT_WINDOW,
	type ExploreQuery,
	MAX_ENTRIES,
	blankQuery,
} from "../query/query";
import type { SelectedLine } from "../query/selection";
import { live, unfinished } from "../runs/status";
import { exactFirst } from "./edit";
import { consolePath, project } from "./queries";

// A threshold with no metric checks the conventional name.
const VALUE = "value";

/** How many of each list blank Explore starts from. */
export const START = { alerts: 3, runs: 4, plots: 3 } as const;

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
	runs: {
		queryKey: ["console", "start", slug, "runs"],
		// The Runs tab's default window, so the landing tab never groups every
		// run; all time only when nothing ran in it.
		queryFn: async () => {
			const runs = async (since = "") =>
				(
					await api.get<JsonConsoleRuns>(
						`${consolePath(slug)}/runs?per_page=${START.runs}${since}`,
					)
				).data.runs;
			const recent = await runs(
				`&start_time=${Date.now() - DEFAULT_WINDOW * 1000}`,
			);
			return recent.length > 0 ? recent : runs();
		},
		...live((runs: JsonConsoleRunSummary[]) =>
			runs.some((run) => unfinished(run.jobs)),
		),
	},
	plots: listQuery<JsonPlot>(
		api,
		slug,
		"plots",
		`/plots?per_page=${START.plots}`,
	),
});

/** The line an alert fired on, as Explore opens it. */
export const alertLine = ({
	threshold,
	benchmark,
	variant,
}: JsonAlert): SelectedLine => ({
	branch: { uuid: threshold.branch.uuid },
	testbed: { uuid: threshold.testbed.uuid },
	benchmark: benchmark.uuid,
	parameters: variant.parameters,
	measure: threshold.measure.uuid,
	metric: threshold.metric ?? VALUE,
	alerting: true,
});

// Explore names a run and how long it took itself: importing the Runs pages'
// helpers splits a chunk off them that every page's entry then names.

/** A run's page. */
export const runHref = (slug: string, run: string) =>
	`${projectPath(slug, "runs")}/${run}`;

/** A run as its link reads: its short hash, else its version number. */
export const runName = (run: { hash?: string | undefined; number: number }) =>
	run.hash?.slice(0, 7) ?? `v${run.number}`;

const pad = (n: number) => String(n).padStart(2, "0");

/** How long a run took, as the Runs list reads it: seconds, then minutes and seconds, then hours and minutes. */
export const runTime = (ms: number) => {
	const seconds = Math.max(0, Math.floor(ms / 1000));
	if (seconds < 60) {
		return `${seconds}s`;
	}
	const minutes = Math.floor(seconds / 60);
	if (minutes < 60) {
		return `${minutes}m ${pad(seconds % 60)}s`;
	}
	return `${Math.floor(minutes / 60)}h ${pad(minutes % 60)}m`;
};

/** A benchmark a report ran, with the measures it reported there. */
export interface ReportBenchmark {
	uuid: string;
	name: string;
	measures: JsonConsoleMeasure[];
}

/** The benchmarks a report ran and the measures each reported, from the groups of the smallest first page of its lines. */
export const reportBenchmarksQuery = (
	api: Api,
	slug: string,
	report: string,
) => ({
	queryKey: ["console", "report-benchmarks", slug, report],
	queryFn: async () =>
		reportBenchmarks(
			(
				await api.get<JsonConsoleReport>(
					`${consolePath(slug)}/reports/${encodeURIComponent(report)}?per_page=1&window=1&points=2`,
				)
			).data,
		),
	// A runner job's report exists before its results, so only an answer that names benchmarks is final.
	staleTime: ({ state }: { state: { data: unknown[] | undefined } }) =>
		state.data?.length ? Number.POSITIVE_INFINITY : 0,
});

/** The benchmarks a report's lines are grouped by, in drawing order. */
export const reportBenchmarks = ({
	groups,
	benchmarks,
	measures,
}: JsonConsoleReport): ReportBenchmark[] =>
	groups.flatMap(({ key, measures: indices }) => {
		const benchmark = benchmarks[key];
		return benchmark === undefined
			? []
			: [
					{
						uuid: benchmark.uuid,
						name: benchmark.name,
						measures: (indices ?? []).flatMap((index) => measures[index] ?? []),
					},
				];
	});

/** A report of a run, with its run's branch. */
export type RunReport = Pick<
	JsonConsoleRunReport,
	"uuid" | "testbed" | "start_time" | "end_time"
> & { branch: Pick<JsonConsoleBranch, "uuid" | "head"> };

/**
 * Explore's query for one benchmark of a report: every variant of it on the
 * report's head and testbed, in the measures it reported, over the default
 * window, or one as long ending at the report when it ran before that window.
 */
export const reportBenchmarkQuery = (
	report: RunReport,
	benchmark: ReportBenchmark,
	now: number,
): ExploreQuery => ({
	...blankQuery(),
	...(report.start_time < now - DEFAULT_WINDOW * 1000
		? { window: { seconds: DEFAULT_WINDOW, end: report.end_time } }
		: {}),
	branches: [{ uuid: report.branch.uuid, head: report.branch.head }],
	testbeds: [{ uuid: report.testbed.uuid }],
	benchmarks: [benchmark.uuid],
	measures: benchmark.measures.slice(0, MAX_ENTRIES).map(({ uuid }) => uuid),
	report: report.uuid,
});

/** The report's benchmarks whose names hold `text`, ignoring case, the one named `text` first. */
export const benchmarkOptions = (
	benchmarks: readonly ReportBenchmark[],
	text: string,
): { value: string; label: string }[] => {
	const lower = text.toLowerCase();
	return exactFirst(
		benchmarks.filter(({ name }) => name.toLowerCase().includes(lower)),
		text,
		({ name }) => name,
	).map(({ uuid, name }) => ({ value: uuid, label: name }));
};
