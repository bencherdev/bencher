import type { JsonConsoleAlertDetail } from "../../types/bencher";
import { type SelectedLine, exploreSearch } from "../query/selection";
import { lineOf } from "./detail";

/** The window an alert's page draws: the 4 weeks ending at its report, as the API reads it. */
const WINDOW_SECONDS = 28 * 86_400;

/** The alert's line as Explore opens it: on the head its report ran on, as the page draws it. */
export const exploreLine = (
	detail: JsonConsoleAlertDetail,
): SelectedLine | undefined => {
	const resolved = lineOf(detail);
	return (
		resolved && {
			branch: { uuid: resolved.branch.uuid, head: resolved.branch.head },
			testbed: { uuid: resolved.testbed.uuid },
			benchmark: resolved.benchmark.uuid,
			parameters: resolved.variant.parameters,
			measure: resolved.measure.uuid,
			metric: resolved.line.metric,
			alerting: detail.status === "active",
		}
	);
};

/** Explore's query string for the alert's line, over the window its page draws. */
export const exploreSearchOf = (detail: JsonConsoleAlertDetail) => {
	const line = exploreLine(detail);
	return (
		line &&
		exploreSearch([line], {
			window: { seconds: WINDOW_SECONDS, end: detail.perf.window.end_time },
			report: detail.report.uuid,
		})
	);
};
