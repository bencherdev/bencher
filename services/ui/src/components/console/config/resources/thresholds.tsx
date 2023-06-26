import THRESHOLD_FIELDS from "./fields/threshold";
import { BENCHER_API_URL } from "../../../site/util";
import { Button, Card, Display, Operation, Row } from "../types";
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
						kind: Row.FOREIGN,
						key: "branch",
					},
					{
						kind: Row.FOREIGN,
						key: "testbed",
					},
					{
						kind: Row.FOREIGN,
						key: "metric_kind",
					},
					{},
				],
				button: {
					text: "View",
					path: (pathname, datum) => viewUuidPath(pathname, datum),
				},
			},
		},
	},
	[Operation.ADD]: {
		operation: Operation.ADD,
		header: {
			title: "Add Threshold",
			path: parentPath,
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
			key: "uuid",
			path: parentPath,
		},
		deck: {
			url: (path_params) =>
				`${BENCHER_API_URL()}/v0/projects/${
					path_params?.project_slug
				}/thresholds/${path_params?.threshold_uuid}`,
			cards: [
				{
					kind: Card.FIELD,
					label: "Branch UUID",
					key: "branch",
					display: Display.RAW,
				},
				{
					kind: Card.FIELD,
					label: "Testbed UUID",
					key: "testbed",
					display: Display.RAW,
				},
				{
					kind: Card.FIELD,
					label: "Metric Kind UUID",
					key: "metric_kind",
					display: Display.RAW,
				},
			],
		},
	},
};

export default thresholdsConfig;
