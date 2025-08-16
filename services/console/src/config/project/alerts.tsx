import type { Params } from "astro";
import { PubResourceKind } from "../../components/perf/util";
import IconTitle from "../../components/site/IconTitle";
import { isAllowedProjectEdit } from "../../util/auth";
import { ActionButton, Button, Card, Display, Operation, Row } from "../types";
import { parentPath, viewUuidPath } from "../util";

export const ALERT_ICON = "fas fa-bell";
export const ALERT_OFF_ICON = "far fa-bell-slash";

const alertsConfig = {
	[Operation.LIST]: {
		operation: Operation.LIST,
		header: {
			title: <IconTitle icon={ALERT_ICON} title="Alerts" />,
			name: "Alerts",
			buttons: [
				{ kind: Button.DISMISS_ALL },
				{ kind: Button.ARCHIVED },
				{ kind: Button.REFRESH },
			],
		},
		table: {
			url: (params: Params) => `/v0/projects/${params?.project}/alerts`,
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
				path: (_pathname: string) => {
					return "https://bencher.dev/docs/explanation/thresholds";
				},
				text: "Learn about Thresholds & Alerts",
			},
			row: {
				kind: Row.ALERT,
				button: {
					text: "View",
					path: viewUuidPath,
				},
			},
			name: "alerts",
		},
	},
	[Operation.VIEW]: {
		operation: Operation.VIEW,
		header: {
			keys: [
				["threshold", "branch", "name"],
				["threshold", "testbed", "name"],
				["benchmark", "name"],
				["threshold", "measure", "name"],
			],
			path: parentPath,
			path_to: "Alerts",
			buttons: [
				{
					kind: Button.STATUS,
					is_allowed: isAllowedProjectEdit,
				},
				{ kind: Button.PERF },
				{ kind: Button.REFRESH },
			],
		},
		deck: {
			url: (params: Params) =>
				`/v0/projects/${params?.project}/alerts/${params?.alert}`,
			top_buttons: [
				{
					kind: ActionButton.ARCHIVED,
					resource: PubResourceKind.Alert,
				},
			],
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
					display: Display.REPORT,
				},
				{
					kind: Card.NESTED_FIELD,
					label: "Branch",
					keys: ["threshold", "branch"],
					display: Display.BRANCH,
				},
				{
					kind: Card.NESTED_FIELD,
					label: "Branch Version Hash",
					keys: ["threshold", "branch", "head", "version", "hash"],
					display: Display.GIT_HASH,
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
					display: Display.FLOAT,
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
					display: Display.FLOAT,
				},
				{
					kind: Card.NESTED_FIELD,
					label: "Lower Boundary Limit",
					keys: ["boundary", "lower_limit"],
					display: Display.FLOAT,
				},
				{
					kind: Card.NESTED_FIELD,
					label: "Upper Boundary Limit",
					keys: ["boundary", "upper_limit"],
					display: Display.FLOAT,
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
			buttons: [
				{
					kind: ActionButton.RAW,
				},
			],
		},
	},
};

export default alertsConfig;
