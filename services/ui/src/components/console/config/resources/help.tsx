import { Operation } from "../types";
import { parentPath } from "../util";

const helpConfig = {
	[Operation.HELP]: {
		operation: Operation.HELP,
		header: {
			title: "Help",
			path: (_pathname) => "/console",
		},
	},
};

export default helpConfig;
