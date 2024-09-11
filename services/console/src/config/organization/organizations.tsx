import type { JsonOrganization } from "../../types/bencher";
import { ActionButton, Button, Card, Display, Row } from "../types";
import { addPath, createdSlugPath, parentPath, viewSlugPath } from "../util";
import { Operation } from "../types";
import type { Params } from "../../util/url";
import {
	isAllowedOrganizationDelete,
	isAllowedOrganizationEdit,
	isAllowedOrganizationManage,
} from "../../util/auth";
import FieldKind from "../../components/field/kind";
import { validResourceName, validOptionJwt, validSlug } from "../../util/valid";
import { isBencherCloud } from "../../util/ext";
import { removeOrganization, setOrganization } from "../../util/organization";

const ORGANIZATION_FIELDS = {
	name: {
		label: "Name",
		type: "text",
		placeholder: "Organization Name",
		icon: "fas fa-project-diagram",
		help: "Must be a non-empty string",
		validate: validResourceName,
	},
	slug: {
		label: "Slug",
		type: "text",
		placeholder: "Organization Slug",
		icon: "fas fa-exclamation-triangle",
		help: "Must be a valid slug",
		validate: validSlug,
	},
	license: {
		label: "License Key",
		type: "text",
		placeholder: "jwt_header.jwt_payload.jwt_verify_signature",
		icon: "fas fa-key",
		help: "Must be a valid JWT (JSON Web Token)",
		validate: validOptionJwt,
	},
};

const organizationsConfig = {
	[Operation.LIST]: {
		operation: Operation.LIST,
		header: {
			title: "Organizations",
			buttons: [
				{ kind: Button.SEARCH },
				{
					kind: Button.ADD,
					title: "Testbed",
					path: addPath,
					is_allowed: async (_apiUrl: string, _params: Params) => true,
				},
				{ kind: Button.REFRESH },
			],
		},
		table: {
			url: (_params: Params) => "/v0/organizations",
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
					effect: (datum: JsonOrganization) => setOrganization(datum),
				},
			},
			name: "organizations",
		},
	},
	[Operation.ADD]: {
		operation: Operation.ADD,
		header: {
			title: "Add Organization",
			path: parentPath,
			path_to: "Organizations",
		},
		form: {
			url: (_params: Params) => "/v0/organizations",
			fields: [
				{
					kind: FieldKind.INPUT,
					label: "Name",
					key: "name",
					value: "",
					valid: null,
					validate: true,
					config: ORGANIZATION_FIELDS.name,
				},
			],
			path: createdSlugPath,
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
				{
					kind: Card.FIELD,
					label: "License Key",
					key: "license",
					display: Display.RAW,
					is_allowed: (apiUrl: string, params: Params) =>
						!isBencherCloud() && isAllowedOrganizationManage(apiUrl, params),
					field: {
						kind: FieldKind.INPUT,
						label: "License Key",
						key: "license",
						value: "",
						valid: null,
						validate: true,
						nullable: true,
						config: ORGANIZATION_FIELDS.license,
					},
				},
			],
			buttons: [
				{
					kind: ActionButton.RAW,
				},
				{
					kind: ActionButton.DELETE,
					subtitle:
						"⚠️ All data associated with this organization will be deleted! ⚠️",
					path: (_pathname: string) => "/console/organizations",
					is_allowed: isAllowedOrganizationDelete,
					effect: () => removeOrganization(),
				},
			],
		},
	},
};

export default organizationsConfig;
