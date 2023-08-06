// import FieldKind from "../../../field/kind";
// import {
// 	BENCHER_API_URL,
// 	OrganizationPermission,
// 	is_allowed_organization,
// } from "../../../site/util";
// import { Button, Card, Display, Operation, Row } from "../types";
// import { parentPath, viewSlugPath } from "../util";
// import ORGANIZATION_FIELDS from "./fields/organization";

import type { Slug } from "../../types/bencher";
import { BENCHER_API_URL } from "../../util/ext";
import { Operation } from "../console";
import { Button, Row } from "../types";
import { viewSlugPath } from "../util";

const organizationsConfig = {
	[Operation.LIST]: {
		operation: Operation.LIST,
		redirect: (tableData: any[]) =>
			tableData?.length === 1
				? `/console/organizations/${tableData[0]?.slug}`
				: null,
		header: {
			title: "Organizations",
			buttons: [{ kind: Button.REFRESH }],
		},
		table: {
			url: (_pathParams: object) => `${BENCHER_API_URL()}/v0/organizations`,
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
					path: (pathname: string, datum: { [slug: string]: Slug }) =>
						viewSlugPath(pathname, datum) + "/projects",
				},
			},
			name: "organizations",
		},
	},
	// [Operation.VIEW]: {
	// 	operation: Operation.VIEW,
	// 	header: {
	// 		key: "name",
	// 		path: parentPath,
	// 		path_to: "Organizations",
	// 		buttons: [{ kind: Button.REFRESH }],
	// 	},
	// 	deck: {
	// 		url: (path_params) =>
	// 			`${BENCHER_API_URL()}/v0/organizations/${
	// 				path_params?.organization_slug
	// 			}`,
	// 		cards: [
	// 			{
	// 				kind: Card.FIELD,
	// 				label: "Organization Name",
	// 				key: "name",
	// 				display: Display.RAW,
	// 				is_allowed: (path_params) =>
	// 					is_allowed_organization(path_params, OrganizationPermission.EDIT),
	// 				field: {
	// 					kind: FieldKind.INPUT,
	// 					label: "Name",
	// 					key: "name",
	// 					value: "",
	// 					valid: null,
	// 					validate: true,
	// 					config: ORGANIZATION_FIELDS.name,
	// 				},
	// 			},
	// 			{
	// 				kind: Card.FIELD,
	// 				label: "Organization Slug",
	// 				key: "slug",
	// 				display: Display.RAW,
	// 				is_allowed: (path_params) =>
	// 					is_allowed_organization(path_params, OrganizationPermission.EDIT),
	// 				field: {
	// 					kind: FieldKind.INPUT,
	// 					label: "Slug",
	// 					key: "slug",
	// 					value: "",
	// 					valid: null,
	// 					validate: true,
	// 					config: ORGANIZATION_FIELDS.slug,
	// 				},
	// 				path: (_path_params, _data) => "/auth/logout",
	// 			},
	// 			{
	// 				kind: Card.FIELD,
	// 				label: "Organization UUID",
	// 				key: "uuid",
	// 				display: Display.RAW,
	// 			},
	// 		],
	// 	},
	// },
};

export default organizationsConfig;
