import type { Params } from "astro";
import { Button, Card, Display } from "../types";
import { PubResourceKind } from "../../components/perf/util";

const benchmarksPubConfig = {
	resource: PubResourceKind.Benchmark,
	header: {
		key: "name",
		buttons: [
			{
				kind: Button.CONSOLE,
				resource: PubResourceKind.Benchmark,
			},
			{ kind: Button.REFRESH },
		],
	},
	deck: {
		url: (params: Params) =>
			`/v0/projects/${params?.project}/benchmarks/${params?.benchmark}`,
		cards: [
			{
				kind: Card.FIELD,
				label: "Benchmark Name",
				key: "name",
				display: Display.RAW,
			},
			{
				kind: Card.FIELD,
				label: "Benchmark Slug",
				key: "slug",
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
};

export default benchmarksPubConfig;
