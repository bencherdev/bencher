import type {
	AlertStatus,
	JsonConsoleAlertLine,
	JsonConsoleAlerts,
} from "../../types/bencher";
import { lineKey } from "../query/line";
import { guardOf } from "../report/delta";
import { type ReportLine, alignSeries } from "../report/lines";
import { shortHash } from "../reports/format";

/** The report that raised an alert, as its group's header names it. */
export interface AlertReport {
	uuid: string;
	start: number;
	created: number;
	hash: string | undefined;
	adapter: string;
	/** Its alerts that match the list's filters and status, over every page. */
	total: number;
}

/** An alert as its row draws it: its line, keyed by the alert, with the report that raised it. */
export interface AlertLine extends ReportLine {
	/** The line's own key, as Explore names it; `key` is the alert's. */
	line: string;
	report: AlertReport;
	status: "active" | "dismissed" | "silenced";
	/** When the alert last changed status, or was raised. */
	modified: number;
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
		const report: AlertReport = {
			uuid: group.uuid,
			start: group.start_time,
			created: group.created,
			hash: shortHash(group.version.hash),
			adapter: group.adapter,
			total: group.total,
		};
		return group.alerts.flatMap(({ line, modified }) => {
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
					modified,
				},
			];
		});
	});

/** A batch with each alert in `statuses` changed to its status at `modified`; the rest keep their objects. */
export const withStatuses = (
	alerts: JsonConsoleAlerts,
	statuses: ReadonlyMap<string, "active" | "dismissed">,
	modified: number,
): JsonConsoleAlerts => {
	// An alert also marks its point in the history of later rows on its line.
	const touched = ({ line }: JsonConsoleAlertLine) =>
		(line.alert !== undefined && statuses.has(line.alert.uuid)) ||
		line.history.alerts.some(({ uuid }) => statuses.has(uuid));
	if (!alerts.groups.some((group) => group.alerts.some(touched))) {
		return alerts;
	}
	const changed = <T extends { uuid: string; status: AlertStatus }>(
		alert: T,
	): T => {
		const status = statuses.get(alert.uuid);
		return status === undefined
			? alert
			: { ...alert, status: status as AlertStatus };
	};
	const rewrite = (entry: JsonConsoleAlertLine): JsonConsoleAlertLine => {
		const { alert, history } = entry.line;
		return {
			modified:
				alert !== undefined && statuses.has(alert.uuid)
					? modified
					: entry.modified,
			line: {
				...entry.line,
				...(alert && { alert: changed(alert) }),
				history: { ...history, alerts: history.alerts.map(changed) },
			},
		};
	};
	return {
		...alerts,
		groups: alerts.groups.map((group) =>
			group.alerts.some(touched)
				? {
						...group,
						alerts: group.alerts.map((entry) =>
							touched(entry) ? rewrite(entry) : entry,
						),
					}
				: group,
		),
	};
};
