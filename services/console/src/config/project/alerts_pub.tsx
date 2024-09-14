import type { Params } from "astro";
import { Button, Card, Display } from "../types";
import { PubResourceKind } from "../../components/perf/util";

const alertsPubConfig = {
	resource: PubResourceKind.Alert,
	header: {
		keys: [
			["threshold", "branch", "name"],
			["threshold", "testbed", "name"],
			["benchmark", "name"],
			["threshold", "measure", "name"],
		],
		buttons: [
			{
				kind: Button.CONSOLE,
				resource: PubResourceKind.Alert,
			},
			{ kind: Button.PERF },
			{ kind: Button.REFRESH },
		],
	},
	deck: {
		url: (params: Params) =>
			`/v0/projects/${params?.project}/alerts/${params?.alert}`,
		cards: [
			{
				kind: Card.NESTED_FIELD,
				label: "Status",
				keys: ["status"],
				display: Display.RAW,
			},
			{
				kind: Card.FIELD,
				label: "Report",
				key: null,
				display: Display.ALERT_REPORT,
			},
			{
				kind: Card.NESTED_FIELD,
				label: "Branch",
				keys: ["threshold", "branch"],
				display: Display.BRANCH,
			},
			{
				kind: Card.NESTED_FIELD,
				label: "Testbed",
				keys: ["threshold", "testbed"],
				display: Display.TESTBED,
			},
			{
				kind: Card.FIELD,
				label: "Benchmark",
				key: "benchmark",
				display: Display.BENCHMARK,
			},
			{
				kind: Card.NESTED_FIELD,
				label: "Measure",
				keys: ["threshold", "measure"],
				display: Display.MEASURE,
			},
			{
				kind: Card.NESTED_FIELD,
				label: "Metric",
				keys: ["metric", "value"],
				display: Display.RAW,
			},
			{
				kind: Card.NESTED_FIELD,
				label: "Boundary Limit Violation",
				keys: ["limit"],
				display: Display.RAW,
			},
			{
				kind: Card.NESTED_FIELD,
				label: "Boundary Baseline",
				keys: ["boundary", "baseline"],
				display: Display.RAW,
			},
			{
				kind: Card.NESTED_FIELD,
				label: "Lower Boundary Limit",
				keys: ["boundary", "lower_limit"],
				display: Display.RAW,
			},
			{
				kind: Card.NESTED_FIELD,
				label: "Upper Boundary Limit",
				keys: ["boundary", "upper_limit"],
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
				label: "Lower Boundary",
				keys: ["threshold", "model", "lower_boundary"],
				display: Display.RAW,
			},
			{
				kind: Card.NESTED_FIELD,
				label: "Upper Boundary",
				keys: ["threshold", "model", "upper_boundary"],
				display: Display.RAW,
			},
			{
				kind: Card.NESTED_FIELD,
				label: "Minimum Sample Size",
				keys: ["threshold", "model", "min_sample_size"],
				display: Display.RAW,
			},
			{
				kind: Card.NESTED_FIELD,
				label: "Maximum Sample Size",
				keys: ["threshold", "model", "max_sample_size"],
				display: Display.RAW,
			},
			{
				kind: Card.NESTED_FIELD,
				label: "Window Size (seconds)",
				keys: ["threshold", "model", "window"],
				display: Display.RAW,
			},
		],
	},
};

export default alertsPubConfig;
