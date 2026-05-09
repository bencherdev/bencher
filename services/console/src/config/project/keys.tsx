import type { Params } from "astro";
import FieldKind from "../../components/field/kind";
import IconTitle from "../../components/site/IconTitle";
import { isAllowedProjectManage } from "../../util/auth";
import { validResourceName, validU32 } from "../../util/valid";
import { ActionButton, Button, Card, Display, Operation } from "../types";
import { addPath, createdUuidPath, parentPath, viewUuidPath } from "../util";

export const KEY_ICON = "fas fa-key";

const KEY_FIELDS = {
	name: {
		type: "text",
		placeholder: "Key Name",
		icon: KEY_ICON,
		help: "Must be a non-empty string",
		validate: validResourceName,
	},
	ttl: {
		type: "number",
		placeholder: "525600",
		icon: "fas fa-stopwatch",
		help: "Must be an integer greater than zero",
		validate: validU32,
	},
};

const keysConfig = {
	[Operation.LIST]: {
		operation: Operation.LIST,
		header: {
			title: <IconTitle icon={KEY_ICON} title="Project Keys" />,
			name: "Project Keys",
			buttons: [
				{ kind: Button.SEARCH },
				{
					kind: Button.ADD,
					title: "Project Key",
					path: addPath,
					is_allowed: isAllowedProjectManage,
				},
				{ kind: Button.REVOKED },
				{ kind: Button.REFRESH },
			],
		},
		table: {
			url: (params: Params) => `/v0/projects/${params?.project}/keys`,
			add: {
				prefix: (
					<div>
						<h4>🐰 Create your first project key!</h4>
						<p>
							It's easy to create a project key.
							<br />
							Tap below to get started.
						</p>
					</div>
				),
				path: addPath,
				text: "Add a Project Key",
			},
			row: {
				key: "name",
				items: [{}, {}, {}, {}],
				button: {
					text: "View",
					path: viewUuidPath,
				},
			},
			name: "keys",
		},
	},
	[Operation.ADD]: {
		operation: Operation.ADD,
		header: {
			title: "Add Project Key",
			path: parentPath,
			path_to: "Project Keys",
		},
		form: {
			url: (params: Params) => `/v0/projects/${params?.project}/keys`,
			fields: [
				{
					kind: FieldKind.INPUT,
					label: "Name",
					key: "name",
					value: "",
					valid: null,
					validate: true,
					config: KEY_FIELDS.name,
				},
				{
					kind: FieldKind.NUMBER,
					label: "Time to Live (TTL) (seconds)",
					key: "ttl",
					value: "",
					valid: true,
					validate: true,
					nullable: true,
					config: KEY_FIELDS.ttl,
				},
			],
			path: createdUuidPath,
		},
	},
	[Operation.VIEW]: {
		operation: Operation.VIEW,
		header: {
			key: "name",
			path: parentPath,
			path_to: "Project Keys",
			buttons: [{ kind: Button.REFRESH }],
		},
		deck: {
			url: (params: Params) =>
				`/v0/projects/${params?.project}/keys/${params?.key}`,
			top_buttons: [
				{
					kind: ActionButton.REVOKED,
					subtitle: "Project Key",
				},
			],
			cards: [
				{
					kind: Card.FIELD,
					label: "Project Key Name",
					key: "name",
					display: Display.RAW,
					is_allowed: isAllowedProjectManage,
					field: {
						kind: FieldKind.INPUT,
						label: "Name",
						key: "name",
						value: "",
						valid: null,
						validate: true,
						config: KEY_FIELDS.name,
					},
				},
				{
					kind: Card.FIELD,
					label: "Project Key UUID",
					key: "uuid",
					display: Display.RAW,
				},
				{
					kind: Card.FIELD,
					label: "Project Key Creation",
					key: "creation",
					display: Display.RAW,
				},
				{
					kind: Card.FIELD,
					label: "Project Key Expiration",
					key: "expiration",
					display: Display.RAW,
				},
				{
					kind: Card.FIELD,
					label: "Project Key Revocation",
					key: "revoked",
					display: Display.RAW,
				},
			],
			buttons: [
				{
					kind: ActionButton.REVOKE,
					subtitle: "Project Key",
					path: (params: Params) =>
						`/v0/projects/${params?.project}/keys/${params?.key}`,
					is_allowed: isAllowedProjectManage,
				},
			],
		},
	},
};

export default keysConfig;
