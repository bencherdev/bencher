import type { Params } from "astro";
import { validEmail, validSlug, validUserName } from "../../util/valid";
import { Button, Card, Display, Operation, Row } from "../types";
import { invitePath, parentPath, viewSlugPath } from "../util";
import { JsonOrganizationPermission } from "../../types/bencher";
import { isAllowedOrganization } from "../../util/auth";
import { BENCHER_API_URL } from "../../util/ext";
import FieldKind from "../../components/field/kind";

const MEMBER_FIELDS = {
	name: {
		type: "text",
		placeholder: "Member Name",
		icon: "fas fa-user",
		help: "May only use: letters, numbers, contained spaces, apostrophes, periods, commas, and dashes",
		validate: validUserName,
	},
	slug: {
		type: "text",
		placeholder: "Member Slug",
		icon: "fas fa-exclamation-triangle",
		help: "Must be at least four characters or longer",
		validate: validSlug,
	},
	email: {
		type: "email",
		placeholder: "email@example.com",
		icon: "fas fa-envelope",
		help: "Must be a valid email address",
		validate: validEmail,
	},
	role: {
		icon: "fas fa-user-tag",
	},
};

const ROLE_VALUE = {
	selected: "leader",
	options: [
		// TODO Team Management
		// {
		//   value: "member",
		//   option: "Member",
		// },
		{
			value: "leader",
			option: "Leader",
		},
	],
};

const MembersConfig = {
	[Operation.LIST]: {
		operation: Operation.LIST,
		header: {
			title: "Organization Members",
			buttons: [
				{
					kind: Button.INVITE,
					title: "Organization",
					path: invitePath,
					is_allowed: (params: Params) =>
						isAllowedOrganization(
							params,
							JsonOrganizationPermission.CreateRole,
						),
				},
				{ kind: Button.REFRESH },
			],
		},
		table: {
			url: (params: Params) =>
				`/v0/organizations/${params?.organization}/members`,
			add: {
				prefix: (
					<div>
						<h4>🐰 Who am I‽</h4>
						<p>
							It's easy to add a new organization member.
							<br />
							Tap below to get started.
						</p>
					</div>
				),
				path: invitePath,
				text: "Invite an Organization Member",
			},
			row: {
				key: "name",
				items: [
					{
						kind: Row.TEXT,
						key: "slug",
					},
					{
						kind: Row.TEXT,
						key: "email",
					},
					{
						kind: Row.SELECT,
						key: "role",
						value: ROLE_VALUE,
					},
					{},
				],
				button: {
					text: "View",
					path: viewSlugPath,
				},
			},
			name: "members",
		},
	},
	[Operation.ADD]: {
		operation: Operation.ADD,
		header: {
			title: "Invite to Organization",
			path: parentPath,
			path_to: "Organization Members",
		},
		form: {
			url: (params: Params) =>
				`/v0/organizations/${params.organization}/members`,
			fields: [
				{
					kind: FieldKind.INPUT,
					label: "Name",
					key: "name",
					value: "",
					valid: null,
					validate: true,
					config: MEMBER_FIELDS.name,
				},
				{
					kind: FieldKind.INPUT,
					label: "Email",
					key: "email",
					value: "",
					valid: null,
					validate: true,
					config: MEMBER_FIELDS.email,
				},
				{
					kind: FieldKind.SELECT,
					label: "Role",
					key: "role",
					value: ROLE_VALUE,
					validate: false,
					config: MEMBER_FIELDS.role,
				},
			],
			button: "Invite",
			path: parentPath,
		},
	},
	[Operation.VIEW]: {
		operation: Operation.VIEW,
		header: {
			key: "name",
			path: parentPath,
			path_to: "Organization Members",
			buttons: [{ kind: Button.REFRESH }],
		},
		deck: {
			url: (params: Params) =>
				`/v0/organizations/${params?.organization}/members/${params?.member}`,
			cards: [
				{
					kind: Card.FIELD,
					label: "Member Name",
					key: "name",
					display: Display.RAW,
				},
				{
					kind: Card.FIELD,
					label: "Member Slug",
					key: "slug",
					display: Display.RAW,
				},
				{
					kind: Card.FIELD,
					label: "Member UUID",
					key: "uuid",
					display: Display.RAW,
				},
				{
					kind: Card.FIELD,
					label: "Member Email",
					key: "email",
					display: Display.RAW,
				},
				{
					kind: Card.FIELD,
					label: "Role",
					key: "role",
					display: Display.SELECT,
					is_allowed: (params: Params) =>
						isAllowedOrganization(params, JsonOrganizationPermission.EditRole),
					field: {
						kind: FieldKind.SELECT,
						key: "role",
						value: ROLE_VALUE,
						validate: false,
						config: MEMBER_FIELDS.role,
					},
				},
			],
		},
	},
};

export default MembersConfig;
