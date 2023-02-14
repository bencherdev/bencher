import FieldKind from "../../../field/kind";
import { BENCHER_API_URL } from "../../../site/util";
import { Button, Card, Display, Operation, Row } from "../types";
import { parentPath, addPath, viewUuidPath } from "../util";
import TOKEN_FIELDS from "./fields/token";

const tokensConfig = {
	[Operation.LIST]: {
		operation: Operation.LIST,
		header: {
			title: "API Tokens",
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
				`${BENCHER_API_URL()}/v0/users/${path_params?.user_slug}/tokens`,
			add: {
				prefix: (
					<div>
						<h4>🐰 Create your first API token!</h4>
						<p>
							It's easy to create an API token.
							<br />
							Tap below to get started.
						</p>
					</div>
				),
				path: addPath,
				text: "Add an API Token",
			},
			row: {
				key: "name",
				items: [{}, {}, {}, {}],
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
			title: "Add API Token",
			path: parentPath,
		},
		form: {
			url: (path_params) =>
				`${BENCHER_API_URL()}/v0/users/${path_params?.user_slug}/tokens`,
			fields: [
				{
					kind: FieldKind.INPUT,
					label: "Name",
					key: "name",
					value: "",
					valid: null,
					validate: true,
					config: TOKEN_FIELDS.name,
				},
				{
					kind: FieldKind.NUMBER,
					label: "Time to Live (TTL) (seconds)",
					key: "ttl",
					value: "",
					valid: true,
					validate: true,
					nullable: true,
					config: TOKEN_FIELDS.ttl,
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
		},
		deck: {
			url: (path_params) =>
				`${BENCHER_API_URL()}/v0/users/${path_params?.user_slug}/tokens/${
					path_params?.token_uuid
				}`,
			cards: [
				{
					kind: Card.FIELD,
					label: "API Token Name",
					key: "name",
					display: Display.RAW,
				},
				{
					kind: Card.FIELD,
					label: "API Token UUID",
					key: "uuid",
					display: Display.RAW,
				},
				{
					kind: Card.FIELD,
					label: "API Token",
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
			],
		},
	},
};

export default tokensConfig;
