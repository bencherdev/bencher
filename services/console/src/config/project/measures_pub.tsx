import type { Params } from "astro";
import { Button, Card, Display } from "../types";
import { PubResourceKind } from "../../components/perf/util";

const measuresPubConfig = {
	resource: PubResourceKind.Measure,
	header: {
		key: "name",
		buttons: [
			{
				kind: Button.CONSOLE,
				resource: PubResourceKind.Measure,
			},
			{ kind: Button.REFRESH },
		],
	},
	deck: {
		url: (params: Params) =>
			`/v0/projects/${params?.project}/measures/${params?.measure}`,
		cards: [
			{
				kind: Card.FIELD,
				label: "Measure Name",
				key: "name",
				display: Display.RAW,
			},
			{
				kind: Card.FIELD,
				label: "Measure Slug",
				key: "slug",
				display: Display.RAW,
			},
			{
				kind: Card.FIELD,
				label: "Measure UUID",
				key: "uuid",
				display: Display.RAW,
			},
			{
				kind: Card.FIELD,
				label: "Measure Units",
				key: "units",
				display: Display.RAW,
			},
		],
	},
};

export default measuresPubConfig;
