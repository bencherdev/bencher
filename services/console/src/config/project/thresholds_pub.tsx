import type { Params } from "astro";
import { Button, Card, Display } from "../types";
import { PubResourceKind } from "../../components/perf/util";

const thresholdsPubConfig = {
	resource: PubResourceKind.Threshold,
	header: {
		keys: [
			["branch", "name"],
			["testbed", "name"],
			["measure", "name"],
		],
		buttons: [
			{
				kind: Button.CONSOLE,
				resource: PubResourceKind.Threshold,
			},
			{ kind: Button.REFRESH },
		],
	},
	deck: {
		url: (params: Params, search: Params) =>
			`/v0/projects/${params?.project}/thresholds/${params?.threshold}${
				search?.model ? `?model=${search?.model}` : ""
			}`,
		cards: [
			{
				kind: Card.FIELD,
				label: "Threshold UUID",
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
				label: "Measure",
				key: "measure",
				display: Display.MEASURE,
			},
			{
				kind: Card.NESTED_FIELD,
				label: "Threshold Model Test",
				keys: ["model", "test"],
				display: Display.RAW,
			},
			{
				kind: Card.NESTED_FIELD,
				label: "Lower Boundary",
				keys: ["model", "lower_boundary"],
				display: Display.RAW,
			},
			{
				kind: Card.NESTED_FIELD,
				label: "Upper Boundary",
				keys: ["model", "upper_boundary"],
				display: Display.RAW,
			},
			{
				kind: Card.NESTED_FIELD,
				label: "Minimum Sample Size",
				keys: ["model", "min_sample_size"],
				display: Display.RAW,
			},
			{
				kind: Card.NESTED_FIELD,
				label: "Maximum Sample Size",
				keys: ["model", "max_sample_size"],
				display: Display.RAW,
			},
			{
				kind: Card.NESTED_FIELD,
				label: "Window Size (seconds)",
				keys: ["model", "window"],
				display: Display.RAW,
			},
		],
	},
};

export default thresholdsPubConfig;
