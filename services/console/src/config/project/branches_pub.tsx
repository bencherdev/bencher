import type { Params } from "astro";
import { PubResourceKind } from "../../components/perf/util";
import {
	Button,
	Card,
	Display,
	ReportDimension,
	ThresholdDimension,
} from "../types";

const branchesPubConfig = {
	resource: PubResourceKind.Branch,
	header: {
		key: "name",
		buttons: [
			{
				kind: Button.CONSOLE,
				resource: PubResourceKind.Branch,
			},
			{ kind: Button.REFRESH },
		],
	},
	deck: {
		url: (params: Params, search: Params) =>
			`/v0/projects/${params?.project}/branches/${params?.branch}${
				search?.head ? `?head=${search?.head}` : ""
			}`,
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
			{
				kind: Card.NESTED_FIELD,
				label: "Branch Version Hash",
				keys: ["head", "version", "hash"],
				display: Display.GIT_HASH,
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
	},
};

export default branchesPubConfig;
