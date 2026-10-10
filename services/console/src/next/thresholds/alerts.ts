import type { AlertStatus, JsonConsoleAlerts } from "../../types/bencher";
import { lineKey } from "../query/line";
import { guardOf } from "../report/delta";
import { type ReportLine, alignSeries } from "../report/lines";

/** An alert as its row draws it: its line, keyed by the alert, with the report that raised it. */
export interface AlertLine extends ReportLine {
	/** The line's own key, as Explore names it; `key` is the alert's. */
	line: string;
	report: { uuid: string; start: number; hash: string | undefined };
	status: `${AlertStatus}`;
}

/** Each alert of a batch, in its group's order, resolved against the batch's tables. */
export const alertLinesOf = (batch: JsonConsoleAlerts): AlertLine[] =>
	batch.groups.flatMap((group) => {
		const branch = batch.branches[group.branch];
		const testbed = batch.testbeds[group.testbed];
		if (!(branch && testbed)) {
			return [];
		}
		const points = {
			x: group.points.x,
			report: group.points.report,
			reports: batch.reports,
		};
		const report = {
			uuid: group.uuid,
			start: group.start_time,
			hash: group.version.hash?.slice(0, 7),
		};
		return group.alerts.flatMap(({ line }) => {
			const benchmark = batch.benchmarks[line.benchmark];
			const variant = batch.variants[line.variant];
			const measure = batch.measures[line.measure];
			if (!(benchmark && variant && measure && line.alert)) {
				return [];
			}
			const model =
				line.model === undefined ? undefined : batch.models[line.model];
			return [
				{
					key: line.alert.uuid,
					line: lineKey({
						branch: branch.uuid,
						testbed: testbed.uuid,
						benchmark: benchmark.uuid,
						parameters: variant.parameters,
						measure: measure.uuid,
						metric: line.metric,
					}),
					branch,
					testbed,
					benchmark,
					variant: variant.uuid,
					parameters: variant.parameters,
					measure,
					metric: line.metric,
					value: line.value,
					baseline: line.baseline,
					lower_limit: line.lower_limit,
					upper_limit: line.upper_limit,
					// A row alerts while its alert is active; the rest say their status.
					alert: line.alert.status === "active" ? line.alert : undefined,
					guard: guardOf(model),
					history: alignSeries(line.history, points.x.length),
					points,
					report,
					status: line.alert.status,
				},
			];
		});
	});
