import type { Params } from "astro";
import { ActionButton, Button, Card, Display, Operation } from "../types";
import { parentPath } from "../util";

const metricsConfig = {
	[Operation.VIEW]: {
		operation: Operation.VIEW,
		header: {
			keys: [
				["branch", "name"],
				["testbed", "name"],
				["benchmark", "name"],
				["measure", "name"],
			],
			path: parentPath,
			path_to: "Plots",
			buttons: [{ kind: Button.REFRESH }],
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
			buttons: [
				{
					kind: ActionButton.RAW,
				},
			],
		},
	},
};

export default metricsConfig;
