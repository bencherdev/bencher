import type { JsonOneMetric } from "../../types/bencher";
import { exploreSearch } from "../query/selection";

/** The window a report's page draws: the 4 weeks ending at its report. */
const WINDOW_SECONDS = 28 * 86_400;

/** Explore's query string for exactly the metric's line, on its report's head, with its report marked. */
export const exploreSearchOf = (metric: JsonOneMetric) =>
	exploreSearch(
		[
			{
				branch: { uuid: metric.branch.uuid, head: metric.branch.head.uuid },
				testbed: { uuid: metric.testbed.uuid },
				benchmark: metric.benchmark.uuid,
				parameters: metric.variant.parameters,
				measure: metric.measure.uuid,
				metric: metric.name,
			},
		],
		{
			window: { seconds: WINDOW_SECONDS, end: Date.parse(metric.end_time) },
			report: metric.report,
		},
	);
