import type { Params } from "astro";
import { Button, Card, Display, Operation } from "../types";
import { parentPath } from "../util";

const branchesPubConfig = {
	operation: Operation.VIEW,
	header: {
		key: "name",
		path: parentPath,
		path_to: "Branches",
		buttons: [{ kind: Button.REFRESH }],
	},
	deck: {
		url: (params: Params) =>
			`/v0/projects/${params?.project}/branches/${params?.branch}`,
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
				kind: Card.FIELD,
				label: "Branch Start Point",
				key: "start_point",
				display: Display.START_POINT,
			},
		],
	},
};

export default branchesPubConfig;
