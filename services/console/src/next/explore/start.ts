import type {
	JsonAlert,
	JsonConsoleMeasure,
	JsonConsoleReport,
	JsonReport,
} from "../../types/bencher";
import type { Api } from "../api";
import {
	DEFAULT_WINDOW,
	type ExploreQuery,
	MAX_ENTRIES,
	blankQuery,
} from "../query/query";
import type { SelectedLine } from "../query/selection";

// A threshold with no metric checks the conventional name.
const VALUE = "value";

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
					`/v0/console/projects/${encodeURIComponent(slug)}/reports/${encodeURIComponent(report)}?per_page=1&window=1&points=2`,
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

/**
 * Explore's query for one benchmark of a report: every variant of it on the
 * report's head and testbed, in the measures it reported, over the default
 * window, or one as long ending at the report when it ran before that window.
 */
export const reportBenchmarkQuery = (
	report: JsonReport,
	benchmark: ReportBenchmark,
	now: number,
): ExploreQuery => ({
	...blankQuery(),
	...(Date.parse(report.start_time) < now - DEFAULT_WINDOW * 1000
		? { window: { seconds: DEFAULT_WINDOW, end: Date.parse(report.end_time) } }
		: {}),
	branches: [{ uuid: report.branch.uuid, head: report.branch.head.uuid }],
	testbeds: [{ uuid: report.testbed.uuid }],
	benchmarks: [benchmark.uuid],
	measures: benchmark.measures.slice(0, MAX_ENTRIES).map(({ uuid }) => uuid),
	report: report.uuid,
});

/** The report's benchmarks whose names hold `text`, ignoring case, at most a menu's worth. */
export const benchmarkOptions = (
	benchmarks: readonly ReportBenchmark[],
	text: string,
	max: number,
): { value: string; label: string }[] => {
	const lower = text.toLowerCase();
	return benchmarks
		.filter(({ name }) => name.toLowerCase().includes(lower))
		.slice(0, max)
		.map(({ uuid, name }) => ({ value: uuid, label: name }));
};
