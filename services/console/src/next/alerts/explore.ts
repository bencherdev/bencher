import {
	type SelectedLine,
	exploreSearch,
	keptLines,
} from "../query/selection";
import type { ReportsWindow } from "../reports/search";
import type { AlertLine } from "./lines";
import { historyDays } from "./search";

const DAY_SECONDS = 86_400;

const selected = (line: AlertLine): SelectedLine => ({
	// The branch, not the head that alerted, so Explore follows it.
	branch: { uuid: line.branch.uuid },
	testbed: { uuid: line.testbed.uuid },
	benchmark: line.benchmark.uuid,
	parameters: line.parameters,
	measure: line.measure.uuid,
	metric: line.metric,
	alerting: line.status === "active",
});

/** Explore's query string for the lines these alerts were raised on, over the page's window. */
export const exploreSearchOf = (
	lines: readonly AlertLine[],
	window: ReportsWindow,
) =>
	exploreSearch(lines.map(selected), {
		window:
			window.kind === "custom"
				? { start: window.start, end: window.end }
				: { seconds: historyDays(window) * DAY_SECONDS },
	});

/** How many of these alerts Explore opens the lines of. */
export const openedOf = (lines: readonly AlertLine[]) =>
	keptLines(lines.map(selected)).length;
