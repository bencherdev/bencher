import type { Params } from "astro";
import { validResourceName, validSlug } from "../../util/valid";
import { ActionButton, Button, Card, Display, Operation, Row } from "../types";
import { parentPath, addPath, viewSlugPath, createdSlugPath } from "../util";
import type { JsonTestbed } from "../../types/bencher";
import FieldKind from "../../components/field/kind";
import {
	isAllowedProjectCreate,
	isAllowedProjectDelete,
	isAllowedProjectEdit,
} from "../../util/auth";

export const TESTBED_ICON = "fas fa-server";

const TESTBED_FIELDS = {
	name: {
		type: "text",
		placeholder: "Testbed Name",
		icon: TESTBED_ICON,
		help: "Must be a non-empty string",
		validate: validResourceName,
	},
	slug: {
		type: "text",
		placeholder: "Testbed Slug",
		icon: "fas fa-exclamation-triangle",
		help: "Must be a valid slug",
		validate: validSlug,
	},
};

const testbedsConfig = {
	[Operation.LIST]: {
		operation: Operation.LIST,
		header: {
			title: "Testbeds",
			buttons: [
				{ kind: Button.SEARCH },
				{ kind: Button.ARCHIVED },
				{
					kind: Button.ADD,
					title: "Testbed",
					path: addPath,
					is_allowed: isAllowedProjectCreate,
				},
				{ kind: Button.REFRESH },
			],
		},
		table: {
			url: (params: Params) => `/v0/projects/${params?.project}/testbeds`,
			add: {
				prefix: (
					<div>
						<h4>
							🐰 Goodbye, <code>localhost</code>!
						</h4>
						<p>
							It's easy to add a new testbed.
							<br />
							Tap below to get started.
						</p>
					</div>
				),
				path: addPath,
				text: "Add a Testbed",
			},
			row: {
				key: "name",
				items: [
					{
						kind: Row.TEXT,
						key: "slug",
					},
					{},
					{},
					{},
				],
				button: {
					text: "View",
					path: viewSlugPath,
				},
			},
			name: "testbeds",
		},
	},
	[Operation.ADD]: {
		operation: Operation.ADD,
		header: {
			title: "Add Testbed",
			path: parentPath,
			path_to: "Testbeds",
		},
		form: {
			url: (params: Params) => `/v0/projects/${params?.project}/testbeds`,
			fields: [
				{
					kind: FieldKind.INPUT,
					label: "Name",
					key: "name",
					value: "",
					valid: null,
					validate: true,
					config: TESTBED_FIELDS.name,
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
			path_to: "Testbeds",
			buttons: [{ kind: Button.REFRESH }],
		},
		deck: {
			url: (params: Params) =>
				`/v0/projects/${params?.project}/testbeds/${params?.testbed}`,
			cards: [
				{
					kind: Card.FIELD,
					label: "Testbed Name",
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
						config: TESTBED_FIELDS.name,
					},
				},
				{
					kind: Card.FIELD,
					label: "Testbed Slug",
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
						config: TESTBED_FIELDS.slug,
					},
					path: (params: Params, data: JsonTestbed) =>
						`/console/projects/${params.project}/testbeds/${data.slug}`,
				},
				{
					kind: Card.FIELD,
					label: "Testbed UUID",
					key: "uuid",
					display: Display.RAW,
				},
			],
			buttons: [
				{
					kind: ActionButton.DELETE,
					subtitle:
						"⚠️ All Reports and Thresholds that use this Testbed must be deleted first! ⚠️",
					path: parentPath,
					is_allowed: isAllowedProjectDelete,
				},
			],
		},
	},
};

export default testbedsConfig;
