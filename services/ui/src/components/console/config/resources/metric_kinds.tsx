import FieldKind from "../../../field/kind";
import {
	BENCHER_API_URL,
	ProjectPermission,
	is_allowed_project,
} from "../../../site/util";
import METRIC_KIND_FIELDS from "./fields/metric_kind";
import { Button, Card, Display, Operation, Row } from "../types";
import { parentPath, addPath, viewSlugPath } from "../util";

const metricKindsConfig = {
	[Operation.LIST]: {
		operation: Operation.LIST,
		header: {
			title: "Metric Kinds",
			buttons: [
				{
					kind: Button.ADD,
					title: "Metric Kind",
					path: addPath,
				},
				{ kind: Button.REFRESH },
			],
		},
		table: {
			url: (path_params) =>
				`${BENCHER_API_URL()}/v0/projects/${
					path_params?.project_slug
				}/metric-kinds`,
			add: {
				prefix: (
					<div>
						<h4>🐰 Who needs units anyway!</h4>
						<p>
							It's easy to create a new metric kind.
							<br />
							Tap below to get started.
						</p>
					</div>
				),
				path: addPath,
				text: "Add a Metric Kind",
			},
			row: {
				key: "name",
				items: [
					{
						kind: Row.TEXT,
						key: "slug",
					},
					{},
					{
						kind: Row.TEXT,
						key: "units",
					},
					{},
				],
				button: {
					text: "View",
					path: (pathname, datum) => viewSlugPath(pathname, datum),
				},
			},
			name: "metric kinds",
		},
	},
	[Operation.ADD]: {
		operation: Operation.ADD,
		header: {
			title: "Add Metric Kind",
			path: parentPath,
			path_to: "Metric Kinds",
		},
		form: {
			url: (path_params) =>
				`${BENCHER_API_URL()}/v0/projects/${
					path_params?.project_slug
				}/metric-kinds`,
			fields: [
				{
					kind: FieldKind.INPUT,
					label: "Name",
					key: "name",
					value: "",
					valid: null,
					validate: true,
					config: METRIC_KIND_FIELDS.name,
				},
				{
					kind: FieldKind.INPUT,
					label: "Units",
					key: "units",
					value: "",
					valid: null,
					validate: true,
					config: METRIC_KIND_FIELDS.units,
				},
			],
			path: parentPath,
		},
	},
	[Operation.VIEW]: {
		operation: Operation.VIEW,
		header: {
			key: "name",
			path: parentPath,
			path_to: "Metric Kinds",
			buttons: [{ kind: Button.REFRESH }],
		},
		deck: {
			url: (path_params) =>
				`${BENCHER_API_URL()}/v0/projects/${
					path_params?.project_slug
				}/metric-kinds/${path_params?.metric_kind_slug}`,
			cards: [
				{
					kind: Card.FIELD,
					label: "Metric Kind Name",
					key: "name",
					display: Display.RAW,
					is_allowed: (path_params) =>
						is_allowed_project(path_params, ProjectPermission.EDIT),
					field: {
						kind: FieldKind.INPUT,
						label: "Name",
						key: "name",
						value: "",
						valid: null,
						validate: true,
						config: METRIC_KIND_FIELDS.name,
					},
				},
				{
					kind: Card.FIELD,
					label: "Metric Kind Slug",
					key: "slug",
					display: Display.RAW,
					is_allowed: (path_params) =>
						is_allowed_project(path_params, ProjectPermission.EDIT),
					field: {
						kind: FieldKind.INPUT,
						label: "Slug",
						key: "slug",
						value: "",
						valid: null,
						validate: true,
						config: METRIC_KIND_FIELDS.slug,
					},
					path: (path_params, data) =>
						`/console/projects/${path_params.project_slug}/metric-kinds/${data.slug}`,
				},
				{
					kind: Card.FIELD,
					label: "Metric Kind UUID",
					key: "uuid",
					display: Display.RAW,
				},
				{
					kind: Card.FIELD,
					label: "Metric Kind Units",
					key: "units",
					display: Display.RAW,
					is_allowed: (path_params) =>
						is_allowed_project(path_params, ProjectPermission.EDIT),
					field: {
						kind: FieldKind.INPUT,
						label: "Units",
						key: "units",
						value: "",
						valid: null,
						validate: true,
						config: METRIC_KIND_FIELDS.units,
					},
				},
			],
		},
	},
};

export default metricKindsConfig;
