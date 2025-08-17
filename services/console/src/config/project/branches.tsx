import type { Params } from "astro";
import FieldKind from "../../components/field/kind";
import IconTitle from "../../components/site/IconTitle";
import type { JsonBranch } from "../../types/bencher";
import {
	isAllowedProjectCreate,
	isAllowedProjectDelete,
	isAllowedProjectEdit,
} from "../../util/auth";
import { validBranchName, validSlug } from "../../util/valid";
import {
	ActionButton,
	Button,
	Card,
	Display,
	Operation,
	ReportDimension,
	Row,
	ThresholdDimension,
} from "../types";
import { addPath, createdSlugPath, parentPath, viewSlugPath } from "../util";

export const BRANCH_ICON = "fas fa-code-branch";

const BRANCH_FIELDS = {
	name: {
		type: "text",
		placeholder: "branch-name",
		icon: BRANCH_ICON,
		help: "Must be a valid git reference",
		validate: validBranchName,
	},
	slug: {
		type: "text",
		placeholder: "Branch Slug",
		icon: "fas fa-exclamation-triangle",
		help: "Must be a valid slug",
		validate: validSlug,
	},
};

const branchesConfig = {
	[Operation.LIST]: {
		operation: Operation.LIST,
		header: {
			title: <IconTitle icon={BRANCH_ICON} title="Branches" />,
			name: "Branches",
			buttons: [
				{ kind: Button.SEARCH },
				{ kind: Button.ARCHIVED },
				{
					kind: Button.ADD,
					title: "Branch",
					path: addPath,
					is_allowed: isAllowedProjectCreate,
				},
				{ kind: Button.REFRESH },
			],
		},
		table: {
			url: (params: Params) => `/v0/projects/${params?.project}/branches`,
			add: {
				prefix: (
					<div>
						<h4>🐰 You did some pruning...</h4>
						<p>
							It's easy to add a new branch.
							<br />
							Tap below to get started.
						</p>
					</div>
				),
				path: addPath,
				text: "Add a Branch",
			},
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
					text: "View",
					path: viewSlugPath,
				},
			},
			name: "branches",
		},
	},
	[Operation.ADD]: {
		operation: Operation.ADD,
		header: {
			title: "Add Branch",
			path: parentPath,
			path_to: "Branches",
		},
		form: {
			url: (params: Params) => `/v0/projects/${params?.project}/branches`,
			fields: [
				{
					kind: FieldKind.INPUT,
					label: "Name",
					key: "name",
					value: "",
					valid: null,
					validate: true,
					config: BRANCH_FIELDS.name,
				},
			],
			path: createdSlugPath,
		},
	},
	[Operation.VIEW]: {
		operation: Operation.VIEW,
		header: {
			key: "name",
			path: parentPath,
			path_to: "Branches",
			buttons: [{ kind: Button.REFRESH }],
		},
		deck: {
			url: (params: Params, search: Params) =>
				`/v0/projects/${params?.project}/branches/${params?.branch}${
					search?.head ? `?head=${search?.head}` : ""
				}`,
			top_buttons: [
				{
					kind: ActionButton.ARCHIVE,
					subtitle: "Branch",
					path: parentPath,
					is_allowed: isAllowedProjectEdit,
				},
				{
					kind: ActionButton.HEAD_REPLACED,
				},
			],
			cards: [
				{
					kind: Card.FIELD,
					label: "Branch Name",
					key: "name",
					display: Display.RAW,
					is_allowed: isAllowedProjectEdit,
					field: {
						kind: FieldKind.INPUT,
						label: "Name",
						key: "name",
						value: "",
						valid: null,
						validate: true,
						config: BRANCH_FIELDS.name,
					},
				},
				{
					kind: Card.FIELD,
					label: "Branch Slug",
					key: "slug",
					display: Display.RAW,
					is_allowed: isAllowedProjectEdit,
					field: {
						kind: FieldKind.INPUT,
						label: "Slug",
						key: "slug",
						value: "",
						valid: null,
						validate: true,
						config: BRANCH_FIELDS.slug,
					},
					path: (params: Params, data: JsonBranch) =>
						`/console/projects/${params.project}/branches/${data.slug}`,
				},
				{
					kind: Card.FIELD,
					label: "Branch UUID",
					key: "uuid",
					display: Display.RAW,
				},
				{
					kind: Card.NESTED_FIELD,
					label: "Branch Start Point",
					keys: ["head", "start_point"],
					display: Display.START_POINT,
				},
				{
					kind: Card.REPORT_TABLE,
					dimension: ReportDimension.BRANCH,
				},
				{
					kind: Card.THRESHOLD_TABLE,
					dimension: ThresholdDimension.BRANCH,
				},
			],
			buttons: [
				{
					kind: ActionButton.RAW,
				},
				{
					kind: ActionButton.DELETE,
					subtitle:
						"⚠️ All Reports and Thresholds that use this Branch must be deleted first! ⚠️",
					path: parentPath,
					is_allowed: isAllowedProjectDelete,
				},
			],
		},
	},
};

export default branchesConfig;
