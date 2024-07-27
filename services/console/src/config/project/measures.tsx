import type { Params } from "astro";
import FieldKind from "../../components/field/kind";
import type { JsonMeasure } from "../../types/bencher";
import {
	isAllowedProjectCreate,
	isAllowedProjectDelete,
	isAllowedProjectEdit,
} from "../../util/auth";
import { validResourceName, validSlug } from "../../util/valid";
import { ActionButton, Button, Card, Display, Operation, Row } from "../types";
import { addPath, createdSlugPath, parentPath, viewSlugPath } from "../util";

export const MEASURE_ICON = "fas fa-shapes";

const MEASURE_FIELDS = {
	name: {
		type: "text",
		placeholder: "Measure Name",
		icon: MEASURE_ICON,
		help: "Must be non-empty string",
		validate: validResourceName,
	},
	slug: {
		type: "text",
		placeholder: "Measure Slug",
		icon: "fas fa-exclamation-triangle",
		help: "Must be a valid slug",
		validate: validSlug,
	},
	units: {
		type: "text",
		placeholder: "units/time",
		icon: "fas fa-ruler",
		help: "Must be non-empty string",
		validate: validResourceName,
	},
};

const measuresConfig = {
	[Operation.LIST]: {
		operation: Operation.LIST,
		header: {
			title: "Measures",
			buttons: [
				{ kind: Button.SEARCH },
				{ kind: Button.ARCHIVED },
				{
					kind: Button.ADD,
					title: "Measure",
					path: addPath,
					is_allowed: isAllowedProjectCreate,
				},
				{ kind: Button.REFRESH },
			],
		},
		table: {
			url: (params: Params) => `/v0/projects/${params?.project}/measures`,
			add: {
				prefix: (
					<div>
						<h4>🐰 Who needs units anyway!</h4>
						<p>
							It's easy to create a new measure.
							<br />
							Tap below to get started.
						</p>
					</div>
				),
				path: addPath,
				text: "Add a Measure",
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
			name: "measures",
		},
	},
	[Operation.ADD]: {
		operation: Operation.ADD,
		header: {
			title: "Add Measure",
			path: parentPath,
			path_to: "Measures",
		},
		form: {
			url: (params: Params) => `/v0/projects/${params?.project}/measures`,
			fields: [
				{
					kind: FieldKind.INPUT,
					label: "Name",
					key: "name",
					value: "",
					valid: null,
					validate: true,
					config: MEASURE_FIELDS.name,
				},
				{
					kind: FieldKind.INPUT,
					label: "Units",
					key: "units",
					value: "",
					valid: null,
					validate: true,
					config: MEASURE_FIELDS.units,
				},
			],
			path: createdSlugPath,
		},
	},
	[Operation.VIEW]: {
		operation: Operation.VIEW,
		header: {
			key: "name",
			path: parentPath,
			path_to: "Measures",
			buttons: [{ kind: Button.REFRESH }],
		},
		deck: {
			url: (params: Params) =>
				`/v0/projects/${params?.project}/measures/${params?.measure}`,
			top_buttons: [
				{
					kind: ActionButton.ARCHIVE,
					subtitle: "Measure",
					path: parentPath,
					is_allowed: isAllowedProjectEdit,
				},
			],
			cards: [
				{
					kind: Card.FIELD,
					label: "Measure Name",
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
						config: MEASURE_FIELDS.name,
					},
				},
				{
					kind: Card.FIELD,
					label: "Measure Slug",
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
						config: MEASURE_FIELDS.slug,
					},
					path: (params: Params, data: JsonMeasure) =>
						`/console/projects/${params.project}/measures/${data.slug}`,
				},
				{
					kind: Card.FIELD,
					label: "Measure UUID",
					key: "uuid",
					display: Display.RAW,
				},
				{
					kind: Card.FIELD,
					label: "Measure Units",
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
						config: MEASURE_FIELDS.units,
					},
				},
			],
			buttons: [
				{
					kind: ActionButton.DELETE,
					subtitle:
						"⚠️ All Reports and Thresholds that use this Measure must be deleted first! ⚠️",
					path: parentPath,
					is_allowed: isAllowedProjectDelete,
				},
			],
		},
	},
};

export default measuresConfig;
