import FieldKind from "../../../field/kind";
import {
	BENCHER_API_URL,
	is_allowed_organization,
	OrganizationPermission,
} from "../../../site/util";
import MEMBER_FIELDS from "./fields/member";
import { Button, Card, Display, Operation, Row } from "../types";
import { parentPath, invitePath, viewSlugPath } from "../util";

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
					path: invitePath,
					is_allowed: (path_params) =>
						is_allowed_organization(
							path_params,
							OrganizationPermission.CREATE_ROLE,
						),
				},
				{ kind: Button.REFRESH },
			],
		},
		table: {
			url: (path_params) =>
				`${BENCHER_API_URL()}/v0/organizations/${
					path_params?.organization_slug
				}/members`,
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
					path: (pathname, datum) => viewSlugPath(pathname, datum),
				},
			},
		},
	},
	[Operation.ADD]: {
		operation: Operation.ADD,
		header: {
			title: "Invite to Organization",
			path: parentPath,
		},
		form: {
			url: (path_params) =>
				`${BENCHER_API_URL()}/v0/organizations/${
					path_params.organization_slug
				}/members`,
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
		},
		deck: {
			url: (path_params) =>
				`${BENCHER_API_URL()}/v0/organizations/${
					path_params?.organization_slug
				}/members/${path_params?.member_slug}`,
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
					is_allowed: (path_params) =>
						is_allowed_organization(
							path_params,
							OrganizationPermission.EDIT_ROLE,
						),
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
