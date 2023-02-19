import { Operation } from "../types";
import { parentPath } from "../util";

const billingConfig = {
	operation: Operation.BILLING,
	header: {
		title: "Billing",
		path: (pathname) => {
			return parentPath(pathname);
		},
	},
};

export default billingConfig;
