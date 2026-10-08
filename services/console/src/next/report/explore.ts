import { type SelectedLine, exploreSearch } from "../query/selection";
import type { ReportLine } from "./lines";

/** A line as Explore opens it, drawn from the report's head. */
export const selectedLine = (line: ReportLine): SelectedLine => ({
	branch: { uuid: line.branch.uuid, head: line.branch.head },
	testbed: { uuid: line.testbed.uuid },
	benchmark: line.benchmark.uuid,
	parameters: line.parameters,
	measure: line.measure.uuid,
	metric: line.metric,
	alerting: line.alert !== undefined,
});

const DAY_SECONDS = 86_400;

/** Explore's query string for exactly these lines, over the window of `days` that ends at their report. */
export const exploreSearchOf = (
	lines: readonly ReportLine[],
	report: { uuid: string; end: number },
	days: number,
) =>
	exploreSearch(lines.map(selectedLine), {
		window: { seconds: days * DAY_SECONDS, end: report.end },
		report: report.uuid,
	});
