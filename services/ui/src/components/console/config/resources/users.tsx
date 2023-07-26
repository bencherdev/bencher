import { BENCHER_API_URL } from "../../../site/util";
import { Button, Card, Display, Operation } from "../types";

const usersConfig = {
	[Operation.VIEW]: {
		operation: Operation.VIEW,
		header: {
			key: "name",
			path: (_pathname) => "/console",
			path_to: "Console Home",
			buttons: [{ kind: Button.REFRESH }],
		},
		deck: {
			url: (path_params) => {
				return `${BENCHER_API_URL()}/v0/users/${path_params?.user_slug}`;
			},
			cards: [
				{
					kind: Card.FIELD,
					label: "Name",
					key: "name",
					display: Display.RAW,
				},
				{
					kind: Card.FIELD,
					label: "Slug",
					key: "slug",
					display: Display.RAW,
				},
				{
					kind: Card.FIELD,
					label: "UUID",
					key: "uuid",
					display: Display.RAW,
				},
				{
					kind: Card.FIELD,
					label: "Email",
					key: "email",
					display: Display.RAW,
				},
			],
		},
	},
};

export default usersConfig;
