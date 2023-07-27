import FieldKind from "../../../field/kind";
import {
	BENCHER_API_URL,
	ProjectPermission,
	is_allowed_project,
} from "../../../site/util";
import { ActionButton, Button, Card, Display, Operation } from "../types";
import { addPath, parentPath, viewSlugPath } from "../util";
import BENCHMARK_FIELDS from "./fields/benchmark";

const benchmarksConfig = {
	[Operation.LIST]: {
		operation: Operation.LIST,
		header: {
			title: "Benchmarks",
			buttons: [
				{
					kind: Button.ADD,
					title: "Benchmark",
					path: addPath,
				},
				{ kind: Button.REFRESH },
			],
		},
		table: {
			url: (path_params) => {
				return `${BENCHER_API_URL()}/v0/projects/${
					path_params?.project_slug
				}/benchmarks`;
			},
			add: {
				prefix: (
					<div>
						<h4>🐰 No benchmarks yet...</h4>
						<p>
							It's easy to track your benchmarks.
							<br />
							Tap below to learn how.
						</p>
					</div>
				),
				path: (_pathname) => {
					return "/docs/how-to/track-benchmarks";
				},
				text: "Track Your Benchmarks",
			},
			row: {
				key: "name",
				items: [{}, {}, {}, {}],
				button: {
					text: "View",
					path: (pathname, datum) => viewSlugPath(pathname, datum),
				},
			},
			name: "benchmarks",
		},
	},
	[Operation.ADD]: {
		operation: Operation.ADD,
		header: {
			title: "Add Benchmark",
			path: parentPath,
			path_to: "Benchmarks",
		},
		form: {
			url: (path_params) =>
				`${BENCHER_API_URL()}/v0/projects/${
					path_params?.project_slug
				}/benchmarks`,
			fields: [
				{
					kind: FieldKind.INPUT,
					label: "Name",
					key: "name",
					value: "",
					valid: null,
					validate: true,
					config: BENCHMARK_FIELDS.name,
				},
			],
			path: parentPath,
		},
	},
	[Operation.VIEW]: {
		operation: Operation.VIEW,
		header: {
			key: "name",
			path: (pathname) => {
				return parentPath(pathname);
			},
			path_to: "Benchmarks",
			buttons: [{ kind: Button.REFRESH }],
		},
		deck: {
			url: (path_params) => {
				return `${BENCHER_API_URL()}/v0/projects/${
					path_params?.project_slug
				}/benchmarks/${path_params?.benchmark_slug}`;
			},
			cards: [
				{
					kind: Card.FIELD,
					label: "Benchmark Name",
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
						config: BENCHMARK_FIELDS.name,
					},
				},
				{
					kind: Card.FIELD,
					label: "Benchmark Slug",
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
						config: BENCHMARK_FIELDS.slug,
					},
					path: (path_params, data) =>
						`/console/projects/${path_params.project_slug}/benchmarks/${data.slug}`,
				},
				{
					kind: Card.FIELD,
					label: "Benchmark UUID",
					key: "uuid",
					display: Display.RAW,
				},
			],
			buttons: [
				{
					kind: ActionButton.DELETE,
					subtitle:
						"⚠️ All Reports that use this Benchmark must be deleted first! ⚠️",
					path: parentPath,
				},
			],
		},
	},
};

export default benchmarksConfig;
