import FieldKind from "../../components/field/kind";
import IconTitle from "../../components/site/IconTitle";
import { isSameUser } from "../../util/auth";
import type { Params } from "../../util/url";
import { validResourceName } from "../../util/valid";
import { ActionButton, Button, Card, Display, Operation } from "../types";
import { parentPath, viewUuidPath } from "../util";

export const TOKEN_ICON = "fas fa-stroopwafel";

const TOKEN_FIELDS = {
	name: {
		type: "text",
		placeholder: "Token Name",
		icon: TOKEN_ICON,
		help: "Must be a non-empty string",
		validate: validResourceName,
	},
};

const tokensConfig = {
	[Operation.LIST]: {
		operation: Operation.LIST,
		header: {
			title: <IconTitle icon={TOKEN_ICON} title="API Tokens" />,
			name: "API Tokens",
			buttons: [
				{ kind: Button.SEARCH },
				{ kind: Button.REVOKED },
				{ kind: Button.REFRESH },
			],
		},
		table: {
			url: (params: Params) => `/v0/users/${params?.user}/tokens`,
			add: {
				prefix: (
					<div>
						<h4>🐰 API tokens are deprecated</h4>
						<p>
							New API tokens can no longer be created.
							<br />
							Use an API key instead.
						</p>
					</div>
				),
				path: (pathname: string) => `${parentPath(pathname)}/keys/add`,
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
			name: "tokens",
		},
	},
	[Operation.VIEW]: {
		operation: Operation.VIEW,
		header: {
			key: "name",
			path: parentPath,
			path_to: "API Tokens",
			buttons: [{ kind: Button.REFRESH }],
		},
		deck: {
			url: (params: Params) =>
				`/v0/users/${params?.user}/tokens/${params?.token}`,
			top_buttons: [
				{
					kind: ActionButton.REVOKED,
					subtitle: "API Token",
				},
			],
			cards: [
				{
					kind: Card.FIELD,
					label: "API Token Name",
					key: "name",
					display: Display.RAW,
					is_allowed: (_params: Params) => true,
					field: {
						kind: FieldKind.INPUT,
						label: "Name",
						key: "name",
						value: "",
						valid: null,
						validate: true,
						config: TOKEN_FIELDS.name,
					},
				},
				{
					kind: Card.FIELD,
					label: "API Token UUID",
					key: "uuid",
					display: Display.RAW,
				},
				{
					kind: Card.FIELD,
					label: (
						<>
							API Token (<code>BENCHER_API_TOKEN</code>)
						</>
					),
					key: "token",
					display: Display.RAW,
				},
				{
					kind: Card.FIELD,
					label: "API Token Creation",
					key: "creation",
					display: Display.RAW,
				},
				{
					kind: Card.FIELD,
					label: "API Token Expiration",
					key: "expiration",
					display: Display.RAW,
				},
				{
					kind: Card.FIELD,
					label: "API Token Revocation",
					key: "revoked",
					display: Display.RAW,
				},
			],
			buttons: [
				{
					kind: ActionButton.REVOKE,
					subtitle: "API Token",
					path: (params: Params) =>
						`/v0/users/${params?.user}/tokens/${params?.token}`,
					is_allowed: isSameUser,
				},
			],
		},
	},
};

export default tokensConfig;
