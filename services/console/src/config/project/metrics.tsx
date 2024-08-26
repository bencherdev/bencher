import type { Params } from "astro";
import { Button, Card, Display, Operation } from "../types";
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
					label: "Report Start Time",
					key: "start_time",
					display: Display.DATE_TIME,
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
					keys: ["branch", "version", "hash"],
					display: Display.GIT_HASH,
				},
				{
					kind: Card.NESTED_FIELD,
					label: "Testbed",
					keys: ["testbed", "name"],
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
					keys: ["measure", "name"],
					display: Display.RAW,
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
	},
};

export default metricsConfig;
