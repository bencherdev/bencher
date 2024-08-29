import type { Params } from "astro";
import { Button, Card, Display } from "../types";

const alertsPubConfig = {
	header: {
		keys: [["benchmark", "name"]],
		buttons: [
			{
				kind: Button.CONSOLE,
				resource: "alerts",
				param: "alert",
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
				kind: Card.NESTED_FIELD,
				label: "Branch",
				keys: ["threshold", "branch", "name"],
				display: Display.RAW,
			},
			{
				kind: Card.NESTED_FIELD,
				label: "Testbed",
				keys: ["threshold", "testbed", "name"],
				display: Display.RAW,
			},
			{
				kind: Card.NESTED_FIELD,
				label: "Benchmark",
				keys: ["benchmark", "name"],
				display: Display.RAW,
			},
			{
				kind: Card.NESTED_FIELD,
				label: "Measure",
				keys: ["threshold", "measure", "name"],
				display: Display.RAW,
			},
			{
				kind: Card.NESTED_FIELD,
				label: "Metric",
				keys: ["benchmark", "metric", "value"],
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
				keys: ["benchmark", "boundary", "baseline"],
				display: Display.RAW,
			},
			{
				kind: Card.NESTED_FIELD,
				label: "Lower Boundary Limit",
				keys: ["benchmark", "boundary", "lower_limit"],
				display: Display.RAW,
			},
			{
				kind: Card.NESTED_FIELD,
				label: "Upper Boundary Limit",
				keys: ["benchmark", "boundary", "upper_limit"],
				display: Display.RAW,
			},
			{
				kind: Card.NESTED_FIELD,
				label: "Threshold Model Test",
				keys: ["threshold", "model", "test"],
				display: Display.RAW,
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
