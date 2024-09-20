import type { Params } from "astro";
import { Button, Card, Display } from "../types";
import { PubResourceKind } from "../../components/perf/util";

const metricsPubConfig = {
	resource: PubResourceKind.Metric,
	header: {
		keys: [
			["branch", "name"],
			["testbed", "name"],
			["benchmark", "name"],
			["measure", "name"],
		],
		buttons: [
			{
				kind: Button.CONSOLE,
				resource: PubResourceKind.Metric,
			},
			{ kind: Button.REFRESH },
		],
	},
	deck: {
		url: (params: Params) =>
			`/v0/projects/${params?.project}/metrics/${params?.metric}`,
		cards: [
			{
				kind: Card.FIELD,
				label: "Report",
				key: null,
				display: Display.REPORT,
			},
			{
				kind: Card.FIELD,
				label: "Branch",
				key: "branch",
				display: Display.BRANCH,
			},
			{
				kind: Card.NESTED_FIELD,
				label: "Branch Version Hash",
				keys: ["branch", "head", "version", "hash"],
				display: Display.GIT_HASH,
			},
			{
				kind: Card.FIELD,
				label: "Testbed",
				key: "testbed",
				display: Display.TESTBED,
			},
			{
				kind: Card.FIELD,
				label: "Benchmark",
				key: "benchmark",
				display: Display.BENCHMARK,
			},
			{
				kind: Card.FIELD,
				label: "Measure",
				key: "measure",
				display: Display.MEASURE,
			},
			{
				kind: Card.NESTED_FIELD,
				label: "Metric Value",
				keys: ["metric", "value"],
				display: Display.RAW,
			},
			{
				kind: Card.NESTED_FIELD,
				label: "Metric Lower Value",
				keys: ["metric", "lower_value"],
				display: Display.RAW,
			},
			{
				kind: Card.NESTED_FIELD,
				label: "Metric Upper Value",
				keys: ["metric", "upper_value"],
				display: Display.RAW,
			},
			{
				kind: Card.FIELD,
				label: "Threshold",
				key: "threshold",
				display: Display.THRESHOLD,
			},
			{
				kind: Card.NESTED_FIELD,
				label: "Threshold Model Test",
				keys: ["threshold", "model", "test"],
				display: Display.MODEL_TEST,
			},
			{
				kind: Card.NESTED_FIELD,
				label: "Boundary Baseline",
				keys: ["boundary", "baseline"],
				display: Display.RAW,
			},
			{
				kind: Card.NESTED_FIELD,
				label: "Boundary Lower Limit",
				keys: ["boundary", "lower_limit"],
				display: Display.RAW,
			},
			{
				kind: Card.NESTED_FIELD,
				label: "Boundary Upper Limit",
				keys: ["boundary", "upper_limit"],
				display: Display.RAW,
			},
		],
	},
};

export default metricsPubConfig;
