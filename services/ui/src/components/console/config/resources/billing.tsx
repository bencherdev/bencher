import { Operation } from "../types";
import { parentPath } from "../util";

export enum Host {
	SELF_HOSTED = "self_hosted",
	BENCHER_CLOUD = "bencher_cloud",
}

const billingConfig = {
	[Host.SELF_HOSTED]: {
		operation: Operation.BILLING,
		header: {
			title: "Billing",
			path: (pathname) => {
				return parentPath(pathname);
			},
		},
	},
	[Host.BENCHER_CLOUD]: {
		operation: Operation.BILLING,
		header: {
			title: "Billing",
			path: (pathname) => {
				return parentPath(pathname);
			},
		},
	},
};

export default billingConfig;
