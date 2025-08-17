import type { Params } from "astro";
import IconTitle from "../../components/site/IconTitle";
import { isAllowedProjectDelete } from "../../util/auth";
import { ActionButton, Button, Card, Display, Operation, Row } from "../types";
import { parentPath, viewUuidPath } from "../util";

export const REPORT_ICON = "far fa-list-alt";

const reportsConfig = {
	[Operation.LIST]: {
		operation: Operation.LIST,
		header: {
			title: <IconTitle icon={REPORT_ICON} title="Reports" />,
			name: "Reports",
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
				kind: Row.REPORT,
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
			display: Display.DATE_TIME,
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
					display: Display.DATE_TIME,
				},
				{
					kind: Card.FIELD,
					label: "Report End Time",
					key: "end_time",
					display: Display.DATE_TIME,
				},
				{
					kind: Card.FIELD,
					label: "Report UUID",
					key: "uuid",
					display: Display.RAW,
				},
				{
					kind: Card.FIELD,
					label: "Branch",
					key: "branch",
					display: Display.BRANCH,
				},
				{
					kind: Card.NESTED_FIELD,
					label: "Branch Version Number",
					keys: ["branch", "head", "version", "number"],
					display: Display.RAW,
				},
				{
					kind: Card.NESTED_FIELD,
					label: "Branch Version Hash",
					keys: ["branch", "head", "version", "hash"],
					display: Display.GIT_HASH,
				},
				{
					kind: Card.FIELD,
					label: "Testbed",
					key: "testbed",
					display: Display.TESTBED,
				},
				{
					kind: Card.FIELD,
					label: "Adapter",
					key: "adapter",
					display: Display.ADAPTER,
				},
				{
					kind: Card.REPORT,
				},
			],
			buttons: [
				{
					kind: ActionButton.RAW,
				},
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
