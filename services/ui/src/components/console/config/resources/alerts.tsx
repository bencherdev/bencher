import { BENCHER_API_URL } from "../../../site/util";
import { Button, Card, Display, Operation, Row } from "../types";
import { parentPath, viewUuidPath } from "../util";

const alertsConfig = {
	[Operation.LIST]: {
		operation: Operation.LIST,
		header: {
			title: "Alerts",
			buttons: [{ kind: Button.REFRESH }],
		},
		table: {
			url: (path_params) => {
				return `${BENCHER_API_URL()}/v0/projects/${
					path_params?.project_slug
				}/alerts`;
			},
			add: {
				prefix: (
					<div>
						<h4>🐰 Good news, no alerts!</h4>
						<p>
							It's easy to track your benchmarks.
							<br />
							Tap below to learn how.
						</p>
					</div>
				),
				path: (_pathname) => {
					return "/docs/explanation/thresholds";
				},
				text: "Learn about Thresholds & Alerts",
			},
			row: {
				keys: [["benchmark", "name"]],
				items: [
					{
						kind: Row.NESTED_TEXT,
						keys: ["threshold", "metric_kind", "name"],
					},
					{
						kind: Row.TEXT,
						key: "status",
					},
					{
						kind: Row.NESTED_TEXT,
						keys: ["threshold", "branch", "name"],
					},
					{
						kind: Row.NESTED_TEXT,
						keys: ["threshold", "testbed", "name"],
					},
				],
				button: {
					text: "View",
					path: (pathname, datum) => {
						return viewUuidPath(pathname, datum);
					},
				},
			},
			name: "alerts",
		},
	},
	[Operation.VIEW]: {
		operation: Operation.VIEW,
		header: {
			keys: [["benchmark", "name"]],
			path: (pathname) => {
				return parentPath(pathname);
			},
		},
		deck: {
			url: (path_params) => {
				return `${BENCHER_API_URL()}/v0/projects/${
					path_params?.project_slug
				}/alerts/${path_params?.alert_uuid}`;
			},
			cards: [
				{
					kind: Card.NESTED_FIELD,
					label: "Status",
					keys: ["status"],
					display: Display.RAW,
				},
				{
					kind: Card.NESTED_FIELD,
					label: "Metric Kind",
					keys: ["threshold", "metric_kind", "name"],
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
					label: "Lower Boundary Limit",
					keys: ["benchmark", "boundary", "lower_limit"],
					display: Display.RAW,
				},
				{
					kind: Card.NESTED_FIELD,
					label: "Upper Boundary",
					keys: ["benchmark", "boundary", "upper_limit"],
					display: Display.RAW,
				},
				{
					kind: Card.NESTED_FIELD,
					label: "Statistical Significance Test",
					keys: ["threshold", "statistic", "test"],
					display: Display.RAW,
				},
				{
					kind: Card.NESTED_FIELD,
					label: "Lower Boundary",
					keys: ["threshold", "statistic", "lower_boundary"],
					display: Display.RAW,
				},
				{
					kind: Card.NESTED_FIELD,
					label: "Upper Boundary",
					keys: ["threshold", "statistic", "upper_boundary"],
					display: Display.RAW,
				},
				{
					kind: Card.NESTED_FIELD,
					label: "Minimum Sample Size",
					keys: ["threshold", "statistic", "min_sample_size"],
					display: Display.RAW,
				},
				{
					kind: Card.NESTED_FIELD,
					label: "Maximum Sample Size",
					keys: ["threshold", "statistic", "max_sample_size"],
					display: Display.RAW,
				},
				{
					kind: Card.NESTED_FIELD,
					label: "Window Size (seconds)",
					keys: ["threshold", "statistic", "window"],
					display: Display.RAW,
				},
			],
		},
	},
};

export default alertsConfig;
