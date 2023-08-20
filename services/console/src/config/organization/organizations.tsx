import type { JsonOrganization } from "../../types/bencher";
import { Button, Card, Display, Row } from "../types";
import { parentPath, viewSlugPath } from "../util";
import { Operation } from "../types";
import type { Params } from "../../util/url";
import { isAllowedOrganizationEdit } from "../../util/auth";
import FieldKind from "../../components/field/kind";
import { validNonEmpty, validSlug } from "../../util/valid";

const ORGANIZATION_FIELDS = {
	name: {
		label: "Name",
		type: "text",
		placeholder: "Organization Name",
		icon: "fas fa-project-diagram",
		help: "Must be a non-empty string",
		validate: validNonEmpty,
	},
	slug: {
		label: "Slug",
		type: "text",
		placeholder: "Organization Slug",
		icon: "fas fa-exclamation-triangle",
		help: "Must be a valid slug",
		validate: validSlug,
	},
};

const organizationsConfig = {
	[Operation.LIST]: {
		operation: Operation.LIST,
		redirect: (tableData: JsonOrganization[]) =>
			tableData?.length === 1
				? `/console/organizations/${tableData[0]?.slug}`
				: null,
		header: {
			title: "Organizations",
			buttons: [{ kind: Button.REFRESH }],
		},
		table: {
			url: (_params: Params) => `/v0/organizations`,
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
					text: "Select",
					path: (pathname: string, datum: JsonOrganization) =>
						`${viewSlugPath(pathname, datum)}/projects`,
				},
			},
			name: "organizations",
		},
	},
	[Operation.VIEW]: {
		operation: Operation.VIEW,
		header: {
			key: "name",
			path: (pathname: string) => `${parentPath(pathname)}/projects`,
			path_to: "Organizations",
			buttons: [{ kind: Button.REFRESH }],
		},
		deck: {
			url: (params: Params) => `/v0/organizations/${params?.organization}`,
			cards: [
				{
					kind: Card.FIELD,
					label: "Organization Name",
					key: "name",
					display: Display.RAW,
					is_allowed: isAllowedOrganizationEdit,
					field: {
						kind: FieldKind.INPUT,
						label: "Name",
						key: "name",
						value: "",
						valid: null,
						validate: true,
						config: ORGANIZATION_FIELDS.name,
					},
				},
				{
					kind: Card.FIELD,
					label: "Organization Slug",
					key: "slug",
					display: Display.RAW,
					is_allowed: isAllowedOrganizationEdit,
					field: {
						kind: FieldKind.INPUT,
						label: "Slug",
						key: "slug",
						value: "",
						valid: null,
						validate: true,
						config: ORGANIZATION_FIELDS.slug,
					},
					path: (_params: Params, _data: JsonOrganization) => "/auth/logout",
				},
				{
					kind: Card.FIELD,
					label: "Organization UUID",
					key: "uuid",
					display: Display.RAW,
				},
			],
		},
	},
};

export default organizationsConfig;
