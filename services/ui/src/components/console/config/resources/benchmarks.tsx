import { BENCHER_API_URL } from "../../../site/util";
import { Button, Card, Display, Operation } from "../types";
import { parentPath, viewUuidPath } from "../util";

const benchmarksConfig = {
	[Operation.LIST]: {
		operation: Operation.LIST,
		header: {
			title: "Benchmarks",
			buttons: [{ kind: Button.REFRESH }],
		},
		table: {
			url: (path_params) => {
				return `${BENCHER_API_URL()}/v0/projects/${
					path_params?.project_slug
				}/benchmarks`;
			},
			add: {
				path: (_pathname) => {
					return "/docs/tutorial/quick-start";
				},
				text: "Run a Report",
			},
			row: {
				key: "name",
				items: [{}, {}, {}, {}],
				button: {
					text: "View",
					path: (pathname, datum) => {
						return viewUuidPath(pathname, datum);
					},
				},
			},
		},
	},
	[Operation.VIEW]: {
		operation: Operation.VIEW,
		header: {
			key: "name",
			path: (pathname) => {
				return parentPath(pathname);
			},
		},
		deck: {
			url: (path_params) => {
				return `${BENCHER_API_URL()}/v0/projects/${
					path_params?.project_slug
				}/benchmarks/${path_params?.benchmark_uuid}`;
			},
			cards: [
				{
					kind: Card.FIELD,
					label: "Benchmark Name",
					key: "name",
					display: Display.RAW,
				},
				{
					kind: Card.FIELD,
					label: "Benchmark UUID",
					key: "uuid",
					display: Display.RAW,
				},
			],
		},
	},
};

export default benchmarksConfig;
