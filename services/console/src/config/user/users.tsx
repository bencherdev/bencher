import { AUTH_FIELDS } from "../../components/auth/auth";
import FieldKind from "../../components/field/kind";
import type { JsonUser } from "../../types/bencher";
import { isSameUser } from "../../util/auth";
import type { Params } from "../../util/url";
import { validSlug } from "../../util/valid";
import { Button, Card, Display, Operation } from "../types";

const USER_FIELDS = {
	slug: {
		type: "text",
		placeholder: "Testbed Slug",
		icon: "fas fa-exclamation-triangle",
		help: "Must be a valid slug",
		validate: validSlug,
	},
};

const usersConfig = {
	[Operation.VIEW]: {
		operation: Operation.VIEW,
		header: {
			key: "name",
			path: (_pathname: string) => "/console",
			path_to: "Console Home",
			buttons: [{ kind: Button.REFRESH }],
		},
		deck: {
			url: (params: Params) => `/v0/users/${params?.user}`,
			cards: [
				{
					kind: Card.FIELD,
					label: "Name",
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
						config: AUTH_FIELDS.username,
					},
				},
				{
					kind: Card.FIELD,
					label: "Slug",
					key: "slug",
					display: Display.RAW,
					is_allowed: isSameUser,
					field: {
						kind: FieldKind.INPUT,
						label: "Slug",
						key: "slug",
						value: "",
						valid: null,
						validate: true,
						config: USER_FIELDS.slug,
					},
					path: (_params: Params, _data: JsonUser) => "/auth/logout",
				},
				{
					kind: Card.FIELD,
					label: "UUID",
					key: "uuid",
					display: Display.RAW,
				},
				{
					kind: Card.FIELD,
					label: "Email",
					key: "email",
					display: Display.RAW,
					is_allowed: isSameUser,
					field: {
						kind: FieldKind.INPUT,
						label: "Email",
						key: "email",
						value: "",
						valid: null,
						validate: true,
						config: AUTH_FIELDS.email,
					},
					path: (_params: Params, _data: JsonUser) => "/auth/logout",
				},
			],
		},
	},
};

export default usersConfig;
