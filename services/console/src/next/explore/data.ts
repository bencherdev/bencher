import {
	AlertStatus,
	type JsonConsoleModel,
	type JsonConsolePerf,
	ModelTest,
} from "../../types/bencher";
import type { PlotData, PlotLine } from "../plot/types";
import { lineKey } from "../query/line";

/** The plot query's answer as the plot draws it, each line named by its key. */
export const plotData = (perf: JsonConsolePerf): PlotData => ({
	x: perf.points.x,
	report: perf.points.report,
	reports: perf.reports,
	measures: perf.measures.map(({ name, units }) => ({ name, units })),
	lines: perf.lines.flatMap((line): PlotLine[] => {
		const branch = perf.branches[line.branch];
		const testbed = perf.testbeds[line.testbed];
		const benchmark = perf.benchmarks[line.benchmark];
		const variant = perf.variants[line.variant];
		const measure = perf.measures[line.measure];
		if (!(branch && testbed && benchmark && variant && measure)) {
			return [];
		}
		const { series } = line;
		const model =
			line.model === undefined ? undefined : perf.models[line.model];
		return [
			{
				id: lineKey({
					branch: branch.uuid,
					testbed: testbed.uuid,
					benchmark: benchmark.uuid,
					parameters: variant.parameters,
					measure: measure.uuid,
					metric: line.metric,
				}),
				benchmark: benchmark.name,
				parameters: variant.parameters,
				measure: line.measure,
				metric: line.metric,
				branch: branch.name,
				testbed: testbed.name,
				alerting: series.alerts.some(
					({ status }) => status === AlertStatus.Active,
				),
				...(model && { model: modelText(model) }),
				y: series.y,
				...(series.baseline && { baseline: series.baseline }),
				...(series.lower && { lower: series.lower }),
				...(series.upper && { upper: series.upper }),
				alerts: series.alerts.map(({ index }) => index),
			},
		];
	}),
});

const modelText = (model: JsonConsoleModel): string => {
	const sides = [
		model.lower_boundary == null ? undefined : "lower",
		model.upper_boundary == null ? undefined : "upper",
	].filter((side) => side !== undefined);
	return [TESTS[model.test], sides.join(" and ")].join(" ").trim();
};

const TESTS: Record<ModelTest, string> = {
	[ModelTest.Static]: "static",
	[ModelTest.Percentage]: "percentage",
	[ModelTest.ZScore]: "z-score",
	[ModelTest.TTest]: "t-test",
	[ModelTest.LogNormal]: "log normal",
	[ModelTest.Iqr]: "IQR",
	[ModelTest.DeltaIqr]: "delta IQR",
};

/** The metric names the answer drew, each once. */
export const metricNames = (perf: JsonConsolePerf): string[] => [
	...new Set(perf.lines.map(({ metric }) => metric)),
];
