import type { Params } from "astro";
import { Button, Card, Display } from "../types";
import { PubResourceKind } from "../../components/perf/util";

const reportsPubConfig = {
	resource: PubResourceKind.Report,
	header: {
		key: "start_time",
		display: Display.DATE_TIME,
		buttons: [
			{
				kind: Button.CONSOLE,
				resource: PubResourceKind.Report,
			},
			{ kind: Button.REFRESH },
		],
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
	},
};

export default reportsPubConfig;
