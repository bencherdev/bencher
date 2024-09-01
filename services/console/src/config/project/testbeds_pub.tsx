import type { Params } from "astro";
import { Button, Card, Display } from "../types";
import { PubResourceKind } from "../../components/perf/util";

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
		],
	},
};

export default testbedsPubConfig;
