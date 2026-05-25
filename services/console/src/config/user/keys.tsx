import FieldKind from "../../components/field/kind";
import IconTitle from "../../components/site/IconTitle";
import { getUserRaw, isSameUser } from "../../util/auth";
import type { Params } from "../../util/url";
import { validNonZeroU32, validResourceName } from "../../util/valid";
import { ActionButton, Button, Card, Display, Operation } from "../types";
import { addPath, createdUuidPath, parentPath, viewUuidPath } from "../util";

export const USER_KEY_ICON = "fas fa-key";

const USER_KEY_FIELDS = {
	name: {
		type: "text",
		placeholder: "Key Name",
		icon: USER_KEY_ICON,
		help: "Must be a non-empty string",
		validate: validResourceName,
	},
	ttl: {
		type: "number",
		placeholder: "525600",
		icon: "fas fa-stopwatch",
		help: "Must be an integer greater than zero",
		validate: validNonZeroU32,
	},
};

const userKeysConfig = {
	[Operation.LIST]: {
		operation: Operation.LIST,
		header: {
			title: <IconTitle icon={USER_KEY_ICON} title="API Keys" />,
			name: "API Keys",
			buttons: [
				{ kind: Button.SEARCH },
				{
					kind: Button.ADD,
					title: "API Key",
					path: addPath,
					is_allowed: async (_apiUrl: string, params: Params) => {
						const user = getUserRaw();
						return (
							user.user.uuid === params?.user ||
							user.user.slug === params?.user ||
							user.user.admin
						);
					},
				},
				{ kind: Button.REVOKED },
				{ kind: Button.REFRESH },
			],
		},
		table: {
			url: (params: Params) => `/v0/users/${params?.user}/keys`,
			add: {
				prefix: (
					<div>
						<h4>🐰 Create your first API key!</h4>
						<p>
							It's easy to create an API key.
							<br />
							Tap below to get started.
						</p>
					</div>
				),
				path: addPath,
				text: "Add an API Key",
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
			title: "Add API Key",
			path: parentPath,
			path_to: "API Keys",
		},
		form: {
			url: (params: Params) => `/v0/users/${params?.user}/keys`,
			fields: [
				{
					kind: FieldKind.INPUT,
					label: "Name",
					key: "name",
					value: "",
					valid: null,
					validate: true,
					config: USER_KEY_FIELDS.name,
				},
				{
					kind: FieldKind.NUMBER,
					label: "Time to Live (TTL) (seconds)",
					key: "ttl",
					value: "",
					valid: true,
					validate: true,
					nullable: true,
					config: USER_KEY_FIELDS.ttl,
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
			path_to: "API Keys",
			buttons: [{ kind: Button.REFRESH }],
		},
		deck: {
			url: (params: Params) =>
				`/v0/users/${params?.user}/keys/${params?.key}`,
			top_buttons: [
				{
					kind: ActionButton.REVOKED,
					subtitle: "API Key",
				},
			],
			cards: [
				{
					kind: Card.FIELD,
					label: "API Key Name",
					key: "name",
					display: Display.RAW,
					is_allowed: isSameUser,
					field: {
						kind: FieldKind.INPUT,
						label: "Name",
						key: "name",
						value: "",
						valid: null,
						validate: true,
						config: USER_KEY_FIELDS.name,
					},
				},
				{
					kind: Card.FIELD,
					label: "API Key UUID",
					key: "uuid",
					display: Display.RAW,
				},
				{
					kind: Card.FIELD,
					label: (
						<>
							API Key (<code>BENCHER_API_KEY</code>)
						</>
					),
					key: "key",
					display: Display.RAW,
				},
				{
					kind: Card.FIELD,
					label: "API Key Creation",
					key: "creation",
					display: Display.RAW,
				},
				{
					kind: Card.FIELD,
					label: "API Key Expiration",
					key: "expiration",
					display: Display.RAW,
				},
				{
					kind: Card.FIELD,
					label: "API Key Revocation",
					key: "revoked",
					display: Display.RAW,
				},
			],
			buttons: [
				{
					kind: ActionButton.REVOKE,
					subtitle: "API Key",
					path: (params: Params) =>
						`/v0/users/${params?.user}/keys/${params?.key}`,
					is_allowed: isSameUser,
				},
			],
		},
	},
};

export default userKeysConfig;
