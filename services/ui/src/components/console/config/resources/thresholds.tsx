import THRESHOLD_FIELDS from "./fields/threshold";
import { BENCHER_API_URL } from "../../../site/util";
import { ActionButton, Button, Card, Display, Operation, Row } from "../types";
import { parentPath, addPath, viewUuidPath } from "../util";
import FieldKind from "../../../field/kind";

const TEST_VALUE = {
	selected: "z",
	options: [
		{
			value: "z",
			option: "Z-score",
		},
		{
			value: "t",
			option: "Student's t-test",
		},
	],
};

const thresholdsConfig = {
	[Operation.LIST]: {
		operation: Operation.LIST,
		header: {
			title: "Thresholds",
			buttons: [
				{
					kind: Button.ADD,
					title: "Threshold",
					path: addPath,
				},
				{ kind: Button.REFRESH },
			],
		},
		table: {
			url: (path_params) =>
				`${BENCHER_API_URL()}/v0/projects/${
					path_params?.project_slug
				}/thresholds`,
			add: {
				prefix: (
					<div>
						<h4>🐰 Create your first threshold!</h4>
						<p>
							It's easy to create a new threshold.
							<br />
							<a href="/docs/explanation/thresholds">Learn about thresholds</a>{" "}
							or tap below to get started.
						</p>
					</div>
				),
				path: addPath,
				text: "Add a Threshold",
			},
			row: {
				keys: [
					["metric_kind", "name"],
					["branch", "name"],
					["testbed", "name"],
				],
				items: [
					{
						kind: Row.NESTED_TEXT,
						keys: ["metric_kind", "name"],
					},
					{},
					{
						kind: Row.NESTED_TEXT,
						keys: ["branch", "name"],
					},
					{
						kind: Row.NESTED_TEXT,
						keys: ["testbed", "name"],
					},
				],
				button: {
					text: "View",
					path: (pathname, datum) => viewUuidPath(pathname, datum),
				},
			},
			name: "thresholds",
		},
	},
	[Operation.ADD]: {
		operation: Operation.ADD,
		header: {
			title: "Add Threshold",
			path: parentPath,
			path_to: "Thresholds",
		},
		form: {
			url: (path_params) =>
				`${BENCHER_API_URL()}/v0/projects/${
					path_params?.project_slug
				}/thresholds`,
			fields: [
				{
					kind: FieldKind.RADIO,
					label: "Metric Kind",
					key: "metric_kind",
					value: "",
					valid: null,
					validate: true,
					config: THRESHOLD_FIELDS.metric_kind,
				},
				{
					kind: FieldKind.RADIO,
					label: "Branch",
					key: "branch",
					value: "",
					valid: null,
					validate: true,
					config: THRESHOLD_FIELDS.branch,
				},
				{
					kind: FieldKind.RADIO,
					label: "Testbed",
					key: "testbed",
					value: "",
					valid: null,
					validate: true,
					config: THRESHOLD_FIELDS.testbed,
				},
				{
					kind: FieldKind.SELECT,
					label: "Statistical Significance Test",
					key: "test",
					value: TEST_VALUE,
					validate: false,
					config: THRESHOLD_FIELDS.test,
				},
				{
					kind: FieldKind.NUMBER,
					label: "Lower Boundary",
					key: "lower_boundary",
					value: "",
					valid: true,
					validate: true,
					nullable: true,
					config: THRESHOLD_FIELDS.lower_boundary,
				},
				{
					kind: FieldKind.NUMBER,
					label: "Upper Boundary",
					key: "upper_boundary",
					value: "",
					valid: true,
					validate: true,
					nullable: true,
					config: THRESHOLD_FIELDS.upper_boundary,
				},
				{
					kind: FieldKind.NUMBER,
					label: "Minimum Sample Size",
					key: "min_sample_size",
					value: "",
					valid: true,
					validate: true,
					nullable: true,
					config: THRESHOLD_FIELDS.min_sample_size,
				},
				{
					kind: FieldKind.NUMBER,
					label: "Maximum Sample Size",
					key: "max_sample_size",
					value: "",
					valid: true,
					validate: true,
					nullable: true,
					config: THRESHOLD_FIELDS.max_sample_size,
				},
				{
					kind: FieldKind.NUMBER,
					label: "Window Size (seconds)",
					key: "window",
					value: "",
					valid: true,
					validate: true,
					nullable: true,
					config: THRESHOLD_FIELDS.window,
				},
			],
			path: parentPath,
		},
	},
	[Operation.VIEW]: {
		operation: Operation.VIEW,
		header: {
			keys: [
				["metric_kind", "name"],
				["branch", "name"],
				["testbed", "name"],
			],
			path: parentPath,
			path_to: "Thresholds",
			buttons: [
				{ kind: Button.EDIT, path: (pathname) => `${pathname}/edit` },
				{ kind: Button.REFRESH },
			],
		},
		deck: {
			url: (path_params) =>
				`${BENCHER_API_URL()}/v0/projects/${
					path_params?.project_slug
				}/thresholds/${path_params?.threshold_uuid}`,
			cards: [
				{
					kind: Card.FIELD,
					label: "Threshold UUID",
					key: "uuid",
					display: Display.RAW,
				},
				{
					kind: Card.NESTED_FIELD,
					label: "Metric Kind",
					keys: ["metric_kind", "name"],
					display: Display.RAW,
				},
				{
					kind: Card.NESTED_FIELD,
					label: "Branch",
					keys: ["branch", "name"],
					display: Display.RAW,
				},
				{
					kind: Card.NESTED_FIELD,
					label: "Testbed",
					keys: ["testbed", "name"],
					display: Display.RAW,
				},
				{
					kind: Card.NESTED_FIELD,
					label: "Statistical Significance Test",
					keys: ["statistic", "test"],
					display: Display.RAW,
				},
				{
					kind: Card.NESTED_FIELD,
					label: "Lower Boundary",
					keys: ["statistic", "lower_boundary"],
					display: Display.RAW,
				},
				{
					kind: Card.NESTED_FIELD,
					label: "Upper Boundary",
					keys: ["statistic", "upper_boundary"],
					display: Display.RAW,
				},
				{
					kind: Card.NESTED_FIELD,
					label: "Minimum Sample Size",
					keys: ["statistic", "min_sample_size"],
					display: Display.RAW,
				},
				{
					kind: Card.NESTED_FIELD,
					label: "Maximum Sample Size",
					keys: ["statistic", "max_sample_size"],
					display: Display.RAW,
				},
				{
					kind: Card.NESTED_FIELD,
					label: "Window Size (seconds)",
					keys: ["statistic", "window"],
					display: Display.RAW,
				},
			],
			buttons: [
				{
					kind: ActionButton.DELETE,
					subtitle:
						"⚠️ All Reports that use this Threshold must be deleted first! ⚠️",
					path: parentPath,
				},
			],
		},
	},
	[Operation.EDIT]: {
		operation: Operation.EDIT,
		header: {
			title: "Edit Threshold Statistic",
			path: parentPath,
			path_to: "Threshold",
		},
		form: {
			url: (path_params) =>
				`${BENCHER_API_URL()}/v0/projects/${
					path_params?.project_slug
				}/thresholds/${path_params?.threshold_uuid}`,
			fields: [
				{
					kind: FieldKind.SELECT,
					label: "Statistical Significance Test",
					key: "test",
					value: TEST_VALUE,
					validate: false,
					config: THRESHOLD_FIELDS.test,
				},
				{
					kind: FieldKind.NUMBER,
					label: "Lower Boundary",
					key: "lower_boundary",
					value: "",
					valid: true,
					validate: true,
					nullable: true,
					config: THRESHOLD_FIELDS.lower_boundary,
				},
				{
					kind: FieldKind.NUMBER,
					label: "Upper Boundary",
					key: "upper_boundary",
					value: "",
					valid: true,
					validate: true,
					nullable: true,
					config: THRESHOLD_FIELDS.upper_boundary,
				},
				{
					kind: FieldKind.NUMBER,
					label: "Minimum Sample Size",
					key: "min_sample_size",
					value: "",
					valid: true,
					validate: true,
					nullable: true,
					config: THRESHOLD_FIELDS.min_sample_size,
				},
				{
					kind: FieldKind.NUMBER,
					label: "Maximum Sample Size",
					key: "max_sample_size",
					value: "",
					valid: true,
					validate: true,
					nullable: true,
					config: THRESHOLD_FIELDS.max_sample_size,
				},
				{
					kind: FieldKind.NUMBER,
					label: "Window Size (seconds)",
					key: "window",
					value: "",
					valid: true,
					validate: true,
					nullable: true,
					config: THRESHOLD_FIELDS.window,
				},
			],
			path: parentPath,
		},
	},
};

export default thresholdsConfig;
