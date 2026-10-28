import type { JsonConsoleAlertDetail } from "../../types/bencher";
import type { Api } from "../api";
import { changeOf, formatMeasure } from "../plot/format";
import { fullName, tagsOf } from "../plot/readout";
import { boundaryText } from "../thresholds/text";

/** An alert, its report, and its line's plot, in the one request its page makes. */
export const alertQuery = (api: Api, slug: string, alert: string) => ({
	queryKey: ["console", "alert", slug, alert] as const,
	queryFn: async ({ signal }: { signal: AbortSignal }) =>
		(
			await api.get<JsonConsoleAlertDetail>(
				`/v0/console/projects/${encodeURIComponent(slug)}/alerts/${encodeURIComponent(alert)}`,
				signal,
			)
		).data,
});

/** The alert's one line, resolved against its answer's tables. */
export const lineOf = ({ perf }: JsonConsoleAlertDetail) => {
	const line = perf.lines[0];
	const branch = line && perf.branches[line.branch];
	const testbed = line && perf.testbeds[line.testbed];
	const benchmark = line && perf.benchmarks[line.benchmark];
	const variant = line && perf.variants[line.variant];
	const measure = line && perf.measures[line.measure];
	return line && branch && testbed && benchmark && variant && measure
		? {
				line,
				branch,
				testbed,
				benchmark,
				variant,
				measure,
				model: line.model === undefined ? undefined : perf.models[line.model],
			}
		: undefined;
};

/** What an alert's page names and states, from the one line its answer holds. */
export const alertFacts = (detail: JsonConsoleAlertDetail) => {
	const resolved = lineOf(detail);
	if (!resolved) {
		return undefined;
	}
	const { line, branch, testbed, benchmark, variant, measure, model } =
		resolved;
	const side = detail.limit;
	const limit = side === "lower" ? detail.lower_limit : detail.upper_limit;
	const lower = model?.lower_boundary != null;
	const upper = model?.upper_boundary != null;
	// As the line's report row says it, from the baseline the threshold compared the value with.
	const delta =
		(lower || upper) && detail.baseline != null
			? changeOf(
					detail.value,
					detail.baseline,
					lower && upper ? "both" : upper ? "upper" : "lower",
				)
			: null;
	const format = (value: number | undefined) =>
		value === undefined ? undefined : formatMeasure(value, measure.units);
	const boundary = model?.[`${side}_boundary`];
	return {
		benchmark: benchmark.name,
		tags: tagsOf(variant.parameters),
		metric: line.metric === "value" ? undefined : line.metric,
		measure: measure.name,
		branch: branch.name,
		testbed: testbed.name,
		name: fullName(
			benchmark.name,
			variant.parameters,
			measure.name,
			line.metric,
		),
		side,
		value: format(detail.value),
		limit: format(limit),
		baseline: format(detail.baseline),
		delta,
		threshold: model && {
			uuid: model.threshold,
			text: [
				model.test,
				...(boundary == null
					? []
					: [`${side}_boundary ${boundaryText(boundary)}`]),
			].join(" · "),
		},
	};
};

export type AlertFacts = NonNullable<ReturnType<typeof alertFacts>>;
