import type { Params } from "astro";
import { ActionButton, Button, Card, Display, Operation, Row } from "../types";
import { parentPath, addPath, viewSlugPath } from "../util";
import type { JsonMetricKind } from "../../types/bencher";
import { BENCHER_API_URL } from "../../util/ext";
import FieldKind from "../../components/field/kind";
import { validNonEmpty, validSlug } from "../../util/valid";
import { isAllowedProjectDelete, isAllowedProjectEdit } from "../../util/auth";

const METRIC_KIND_FIELDS = {
	name: {
		type: "text",
		placeholder: "Metric Kind Name",
		icon: "fas fa-shapes",
		help: "Must be non-empty string",
		validate: validNonEmpty,
	},
	slug: {
		type: "text",
		placeholder: "Metric Kind Slug",
		icon: "fas fa-exclamation-triangle",
		help: "Must be a valid slug",
		validate: validSlug,
	},
	units: {
		type: "text",
		placeholder: "units/time",
		icon: "fas fa-ruler",
		help: "Must be non-empty string",
		validate: validNonEmpty,
	},
};

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
			url: (params: Params) =>
				`${BENCHER_API_URL()}/v0/projects/${params?.project}/metric-kinds`,
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
					path: viewSlugPath,
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
			url: (params: Params) =>
				`${BENCHER_API_URL()}/v0/projects/${params?.project}/metric-kinds`,
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
			url: (params: Params) =>
				`${BENCHER_API_URL()}/v0/projects/${params?.project}/metric-kinds/${
					params?.metric_kind
				}`,
			cards: [
				{
					kind: Card.FIELD,
					label: "Metric Kind Name",
					key: "name",
					display: Display.RAW,
					is_allowed: isAllowedProjectEdit,
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
					is_allowed: isAllowedProjectEdit,
					field: {
						kind: FieldKind.INPUT,
						label: "Slug",
						key: "slug",
						value: "",
						valid: null,
						validate: true,
						config: METRIC_KIND_FIELDS.slug,
					},
					path: (params: Params, data: JsonMetricKind) =>
						`/console/projects/${params.project}/metric-kinds/${data.slug}`,
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
					is_allowed: isAllowedProjectEdit,
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
			buttons: [
				{
					kind: ActionButton.DELETE,
					subtitle:
						"⚠️ All Reports and Thresholds that use this Metric Kind must be deleted first! ⚠️",
					path: parentPath,
					is_allowed: isAllowedProjectDelete,
				},
			],
		},
	},
};

export default metricKindsConfig;
