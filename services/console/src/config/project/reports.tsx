import type { Params } from "astro";
import { ActionButton, Button, Card, Display, Operation, Row } from "../types";
import { parentPath, viewUuidPath } from "../util";
import { isAllowedProjectDelete } from "../../util/auth";

const reportsConfig = {
	[Operation.LIST]: {
		operation: Operation.LIST,
		header: {
			title: "Reports",
			buttons: [
				{ kind: Button.DATE_TIME },
				{ kind: Button.ARCHIVED },
				{ kind: Button.REFRESH },
			],
		},
		table: {
			url: (params: Params) => `/v0/projects/${params?.project}/reports`,
			add: {
				prefix: (
					<div>
						<h4>🐰 No reports yet...</h4>
						<p>
							It's easy to track your benchmarks.
							<br />
							Tap below to learn how.
						</p>
					</div>
				),
				path: (_pathname: string) =>
					"https://bencher.dev/docs/how-to/track-benchmarks",
				text: "Track Your Benchmarks",
			},
			row: {
				key: "start_time",
				kind: Row.DATE_TIME,
				items: [
					{
						kind: Row.TEXT,
						key: "adapter",
					},
					{},
					{
						kind: Row.NESTED_TEXT,
						keys: ["branch", "name"],
					},
					{
						kind: Row.NESTED_TEXT,
						keys: ["testbed", "name"],
					},
				],
				button: {
					text: "View",
					path: viewUuidPath,
				},
			},
			name: "reports",
		},
	},
	[Operation.VIEW]: {
		operation: Operation.VIEW,
		header: {
			key: "start_time",
			path: parentPath,
			path_to: "Reports",
			buttons: [{ kind: Button.REFRESH }],
		},
		deck: {
			url: (params: Params) =>
				`/v0/projects/${params?.project}/reports/${params?.report}`,
			cards: [
				{
					kind: Card.FIELD,
					label: "Report Start Time",
					key: "start_time",
					display: Display.RAW,
				},
				{
					kind: Card.FIELD,
					label: "Report End Time",
					key: "end_time",
					display: Display.RAW,
				},
				{
					kind: Card.FIELD,
					label: "Report UUID",
					key: "uuid",
					display: Display.RAW,
				},
				{
					kind: Card.FIELD,
					label: "Results Adapter",
					key: "adapter",
					display: Display.RAW,
				},
			],
			buttons: [
				{
					kind: ActionButton.DELETE,
					subtitle: null,
					path: parentPath,
					is_allowed: isAllowedProjectDelete,
				},
			],
		},
	},
};

export default reportsConfig;
