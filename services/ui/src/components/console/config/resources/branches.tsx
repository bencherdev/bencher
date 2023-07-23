import BRANCH_FIELDS from "./fields/branch";
import { Button, Card, Display, Operation, Row } from "../types";
import { parentPath, addPath, viewSlugPath } from "../util";
import { BENCHER_API_URL } from "../../../site/util";
import FieldKind from "../../../field/kind";

const branchesConfig = {
	[Operation.LIST]: {
		operation: Operation.LIST,
		header: {
			title: "Branches",
			buttons: [
				{
					kind: Button.ADD,
					title: "Branch",
					path: addPath,
				},
				{ kind: Button.REFRESH },
			],
		},
		table: {
			url: (path_params) =>
				`${BENCHER_API_URL()}/v0/projects/${
					path_params?.project_slug
				}/branches`,
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
					path: (pathname, datum) => viewSlugPath(pathname, datum),
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
			url: (path_params) =>
				`${BENCHER_API_URL()}/v0/projects/${
					path_params?.project_slug
				}/branches`,
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
			path: parentPath,
		},
	},
	[Operation.VIEW]: {
		operation: Operation.VIEW,
		header: {
			key: "name",
			path: parentPath,
			path_to: "Branches",
		},
		deck: {
			url: (path_params) =>
				`${BENCHER_API_URL()}/v0/projects/${
					path_params?.project_slug
				}/branches/${path_params?.branch_slug}`,
			cards: [
				{
					kind: Card.FIELD,
					label: "Branch Name",
					key: "name",
					display: Display.RAW,
				},
				{
					kind: Card.FIELD,
					label: "Branch Slug",
					key: "slug",
					display: Display.RAW,
				},
				{
					kind: Card.FIELD,
					label: "Branch UUID",
					key: "uuid",
					display: Display.RAW,
				},
				// {
				//   kind: Card.TABLE,
				//   label: "Versions",
				//   key: "versions",
				// },
			],
		},
	},
};

export default branchesConfig;
