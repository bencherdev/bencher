import { BENCHER_API_URL } from "../../../site/util";
import { Button, Card, Display, Operation } from "../types";
import { parentPath, viewUuidPath } from "../util";

const alertsConfig = {
	[Operation.LIST]: {
		operation: Operation.LIST,
		header: {
			title: "Alerts",
			buttons: [{ kind: Button.REFRESH }],
		},
		table: {
			url: (path_params) => {
				return `${BENCHER_API_URL()}/v0/projects/${
					path_params?.project_slug
				}/alerts`;
			},
			add: {
				prefix: (
					<div>
						<h4>🐰 Good news, no alerts!</h4>
						<p>
							It's easy to run your benchmarks.
							<br />
							Tap below to learn how.
						</p>
					</div>
				),
				path: (_pathname) => {
					return "/docs/tutorial/quick-start";
				},
				text: "Run Your Benchmarks",
			},
			row: {
				key: "uuid",
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
			key: "uuid",
			path: (pathname) => {
				return parentPath(pathname);
			},
		},
		deck: {
			url: (path_params) => {
				return `${BENCHER_API_URL()}/v0/projects/${
					path_params?.project_slug
				}/alerts/${path_params?.alert_uuid}`;
			},
			cards: [
				{
					kind: Card.FIELD,
					label: "Perf UUID",
					key: "perf",
					display: Display.RAW,
				},
				{
					kind: Card.FIELD,
					label: "Threshold UUID",
					key: "threshold",
					display: Display.RAW,
				},
				{
					kind: Card.FIELD,
					label: "Statistic UUID",
					key: "statistic",
					display: Display.RAW,
				},
				{
					kind: Card.FIELD,
					label: "Side",
					key: "side",
					display: Display.RAW,
				},
				{
					kind: Card.FIELD,
					label: "Boundary",
					key: "boundary",
					display: Display.RAW,
				},
				{
					kind: Card.FIELD,
					label: "Outlier",
					key: "outlier",
					display: Display.RAW,
				},
			],
		},
	},
};

export default alertsConfig;
