import type { JsonConsoleAlerts } from "../../types/bencher";
import { lineKey } from "../query/line";
import { guardOf } from "../report/delta";
import { type ReportLine, alignSeries } from "../report/lines";
import { shortHash } from "../reports/format";

/** The report that raised an alert, as its group's header names it. */
export interface AlertReport {
	uuid: string;
	/** Its place in its run by start, in a run's alerts. */
	order: number | undefined;
	start: number;
	end: number;
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

// The line each alert resolved to, and what it was resolved from: a batch read
// again with the same answer for an alert keeps the alert's objects, so its row
// keeps its line, its open plot, and the focus.
const resolved = new WeakMap<
	JsonConsoleAlerts["groups"][number]["alerts"][number],
	{ line: AlertLine; from: readonly unknown[] }
>();

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
			...(group.points.iteration && { iteration: group.points.iteration }),
			reports: batch.reports,
		};
		const report: AlertReport = {
			uuid: group.uuid,
			order: group.order,
			start: group.start_time,
			end: group.end_time,
			created: group.created,
			hash: shortHash(group.version.hash),
			adapter: group.adapter,
			total: group.total,
		};
		return group.alerts.flatMap((entry) => {
			const { line, modified } = entry;
			const benchmark = batch.benchmarks[line.benchmark];
			const variant = batch.variants[line.variant];
			const measure = batch.measures[line.measure];
			if (!(benchmark && variant && measure && line.alert)) {
				return [];
			}
			const model =
				line.model === undefined ? undefined : batch.models[line.model];
			const { total, ...drawn } = report;
			const from = [
				branch,
				testbed,
				benchmark,
				variant,
				measure,
				model,
				points.x,
				points.report,
				points.reports,
				...Object.values(drawn),
			];
			const known = resolved.get(entry);
			if (
				known &&
				known.from.length === from.length &&
				known.from.every((part, index) => part === from[index])
			) {
				// The total is its group header's alone, so the row keeps its object.
				known.line.report = report;
				return [known.line];
			}
			const resolvedLine: AlertLine = {
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
			};
			resolved.set(entry, { line: resolvedLine, from });
			return [resolvedLine];
		});
	});
