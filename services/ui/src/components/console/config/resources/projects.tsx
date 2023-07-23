import PROJECT_FIELDS from "./fields/project";
import {
	BENCHER_API_URL,
	is_allowed_project,
	ProjectPermission,
} from "../../../site/util";
import { Button, Card, Display, Operation, PerfTab, Row } from "../types";
import { parentPath, addPath } from "../util";
import FieldKind from "../../../field/kind";

const VISIBILITY_VALUE = {
	selected: "public",
	options: [
		{
			value: "public",
			option: "Public",
		},
		{
			value: "private",
			option: "Private",
		},
	],
};

const projectsConfig = {
	[Operation.LIST]: {
		operation: Operation.LIST,
		header: {
			title: "Projects",
			buttons: [
				{
					kind: Button.ADD,
					title: "Project",
					path: addPath,
				},
				{ kind: Button.REFRESH },
			],
		},
		table: {
			url: (path_params) =>
				`${BENCHER_API_URL()}/v0/organizations/${
					path_params?.organization_slug
				}/projects`,
			add: {
				prefix: (
					<div>
						<h4>🐰 Create your first project!</h4>
						<p>
							It's easy to create a project.
							<br />
							Tap below to get started.
						</p>
					</div>
				),
				path: addPath,
				text: "Add a Project",
			},
			row: {
				key: "name",
				items: [
					{
						kind: Row.TEXT,
						key: "slug",
					},
					{},
					{
						kind: Row.TEXT,
						key: "visibility",
					},
					{},
				],
				button: {
					text: "Select",
					path: (_pathname, datum) => `/console/projects/${datum?.slug}/perf`,
				},
			},
			name: "projects",
		},
	},
	[Operation.ADD]: {
		operation: Operation.ADD,
		header: {
			title: "Add Project",
			path: parentPath,
			path_to: "Projects",
		},
		form: {
			url: (path_params) =>
				`${BENCHER_API_URL()}/v0/organizations/${
					path_params.organization_slug
				}/projects`,
			fields: [
				{
					kind: FieldKind.INPUT,
					label: "Name",
					key: "name",
					value: "",
					valid: null,
					validate: true,
					config: PROJECT_FIELDS.name,
				},
				{
					kind: FieldKind.INPUT,
					label: "URL",
					key: "url",
					value: "",
					valid: true,
					validate: true,
					nullable: true,
					config: PROJECT_FIELDS.url,
				},
				{
					kind: FieldKind.SELECT,
					label: "Visibility",
					key: "visibility",
					value: VISIBILITY_VALUE,
					validate: false,
					config: PROJECT_FIELDS.visibility,
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
			path_to: "Projects",
		},
		deck: {
			url: (path_params) =>
				`${BENCHER_API_URL()}/v0/projects/${path_params?.project_slug}`,
			cards: [
				{
					kind: Card.FIELD,
					label: "Project Name",
					key: "name",
					display: Display.RAW,
					is_allowed: (path_params) =>
						is_allowed_project(path_params, ProjectPermission.EDIT),
					field: {
						kind: FieldKind.INPUT,
						label: "Name",
						key: "name",
						value: "",
						valid: null,
						validate: true,
						config: PROJECT_FIELDS.name,
					},
				},
				{
					kind: Card.FIELD,
					label: "Project Slug",
					key: "slug",
					display: Display.RAW,
					is_allowed: (path_params) =>
						is_allowed_project(path_params, ProjectPermission.EDIT),
					field: {
						kind: FieldKind.INPUT,
						label: "Slug",
						key: "slug",
						value: "",
						valid: null,
						validate: true,
						config: PROJECT_FIELDS.slug,
					},
					path: (_path_params, data) =>
						`/console/projects/${data?.slug}/settings`,
				},
				{
					kind: Card.FIELD,
					label: "Project UUID",
					key: "uuid",
					display: Display.RAW,
				},
				{
					kind: Card.FIELD,
					label: "Project URL",
					key: "url",
					display: Display.RAW,
					is_allowed: (path_params) =>
						is_allowed_project(path_params, ProjectPermission.EDIT),
					field: {
						kind: FieldKind.INPUT,
						label: "URL",
						key: "url",
						value: "",
						valid: null,
						validate: true,
						nullable: true,
						config: PROJECT_FIELDS.url,
					},
				},
				{
					kind: Card.FIELD,
					label: "Visibility",
					key: "visibility",
					display: Display.SELECT,
					is_allowed: (path_params) =>
						is_allowed_project(path_params, ProjectPermission.EDIT),
					field: {
						kind: FieldKind.SELECT,
						key: "visibility",
						value: VISIBILITY_VALUE,
						validate: false,
						config: PROJECT_FIELDS.visibility,
					},
				},
			],
		},
	},
	[Operation.PERF]: {
		operation: Operation.PERF,
		header: {
			url: (project_slug) =>
				`${BENCHER_API_URL()}/v0/projects/${project_slug}/perf/img`,
		},
		plot: {
			project_url: (project_slug) =>
				`${BENCHER_API_URL()}/v0/projects/${project_slug}`,
			perf_url: (project_slug) =>
				`${BENCHER_API_URL()}/v0/projects/${project_slug}/perf`,
			tab_url: (project_slug, tab: PerfTab) =>
				`${BENCHER_API_URL()}/v0/projects/${project_slug}/${tab}`,
			metric_kinds_url: (project_slug) =>
				`${BENCHER_API_URL()}/v0/projects/${project_slug}/metric-kinds`,
		},
	},
};

export default projectsConfig;
