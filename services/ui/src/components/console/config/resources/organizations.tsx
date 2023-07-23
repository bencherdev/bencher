import FieldKind from "../../../field/kind";
import {
	BENCHER_API_URL,
	OrganizationPermission,
	is_allowed_organization,
} from "../../../site/util";
import { Button, Card, Display, Operation, Row } from "../types";
import { parentPath, viewSlugPath } from "../util";
import ORGANIZATION_FIELDS from "./fields/organization";

const organizationsConfig = {
	[Operation.LIST]: {
		operation: Operation.LIST,
		redirect: (table_data) =>
			table_data?.length === 1
				? `/console/organizations/${table_data[0]?.slug}`
				: null,
		header: {
			title: "Organizations",
			buttons: [{ kind: Button.REFRESH }],
		},
		table: {
			url: (_path_params) => `${BENCHER_API_URL()}/v0/organizations`,
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
					path: (pathname, datum) =>
						viewSlugPath(pathname, datum) + "/projects",
				},
			},
			name: "organizations",
		},
	},
	[Operation.VIEW]: {
		operation: Operation.VIEW,
		header: {
			key: "name",
			path: parentPath,
			path_to: "Organizations",
		},
		deck: {
			url: (path_params) =>
				`${BENCHER_API_URL()}/v0/organizations/${
					path_params?.organization_slug
				}`,
			cards: [
				{
					kind: Card.FIELD,
					label: "Organization Name",
					key: "name",
					display: Display.RAW,
					is_allowed: (path_params) =>
						is_allowed_organization(path_params, OrganizationPermission.EDIT),
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
					is_allowed: (path_params) =>
						is_allowed_organization(path_params, OrganizationPermission.EDIT),
					field: {
						kind: FieldKind.INPUT,
						label: "Slug",
						key: "slug",
						value: "",
						valid: null,
						validate: true,
						config: ORGANIZATION_FIELDS.slug,
					},
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
