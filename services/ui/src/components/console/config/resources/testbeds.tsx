import FieldKind from "../../../field/kind";
import {
	BENCHER_API_URL,
	ProjectPermission,
	is_allowed_project,
} from "../../../site/util";
import TESTBED_FIELDS from "./fields/testbed";
import { Button, Card, Display, Operation, Row } from "../types";
import { parentPath, addPath, viewSlugPath } from "../util";

const testbedsConfig = {
	[Operation.LIST]: {
		operation: Operation.LIST,
		header: {
			title: "Testbeds",
			buttons: [
				{
					kind: Button.ADD,
					title: "Testbed",
					path: addPath,
				},
				{ kind: Button.REFRESH },
			],
		},
		table: {
			url: (path_params) =>
				`${BENCHER_API_URL()}/v0/projects/${
					path_params?.project_slug
				}/testbeds`,
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
					path: (pathname, datum) => viewSlugPath(pathname, datum),
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
			url: (path_params) =>
				`${BENCHER_API_URL()}/v0/projects/${
					path_params?.project_slug
				}/testbeds`,
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
			path: parentPath,
		},
	},
	[Operation.VIEW]: {
		operation: Operation.VIEW,
		header: {
			key: "name",
			path: parentPath,
			path_to: "Testbeds",
		},
		deck: {
			url: (path_params) =>
				`${BENCHER_API_URL()}/v0/projects/${
					path_params?.project_slug
				}/testbeds/${path_params?.testbed_slug}`,
			cards: [
				{
					kind: Card.FIELD,
					label: "Testbed Name",
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
						config: TESTBED_FIELDS.name,
					},
				},
				{
					kind: Card.FIELD,
					label: "Testbed Slug",
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
						config: TESTBED_FIELDS.slug,
					},
					path: (path_params, data) =>
						`/console/projects/${path_params.project_slug}/testbeds/${data.slug}`,
				},
				{
					kind: Card.FIELD,
					label: "Testbed UUID",
					key: "uuid",
					display: Display.RAW,
				},
			],
		},
	},
};

export default testbedsConfig;
