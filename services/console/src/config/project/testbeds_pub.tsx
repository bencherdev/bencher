import type { Params } from "astro";
import { PubResourceKind } from "../../components/perf/util";
import {
	Button,
	Card,
	Display,
	ReportDimension,
	ThresholdDimension,
} from "../types";

const testbedsPubConfig = {
	resource: PubResourceKind.Testbed,
	header: {
		key: "name",
		buttons: [
			{
				kind: Button.CONSOLE,
				resource: PubResourceKind.Testbed,
			},
			{ kind: Button.REFRESH },
		],
	},
	deck: {
		url: (params: Params) =>
			`/v0/projects/${params?.project}/testbeds/${params?.testbed}`,
		cards: [
			{
				kind: Card.FIELD,
				label: "Testbed Name",
				key: "name",
				display: Display.RAW,
			},
			{
				kind: Card.FIELD,
				label: "Testbed Slug",
				key: "slug",
				display: Display.RAW,
			},
			{
				kind: Card.FIELD,
				label: "Testbed UUID",
				key: "uuid",
				display: Display.RAW,
			},
			{
				kind: Card.REPORT_TABLE,
				dimension: ReportDimension.TESTBED,
			},
			{
				kind: Card.THRESHOLD_TABLE,
				dimension: ThresholdDimension.TESTBED,
			},
		],
	},
};

export default testbedsPubConfig;
