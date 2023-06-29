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
			title: "License Billing",
			path: (pathname) => {
				return parentPath(pathname);
			},
		},
		host: Host.SELF_HOSTED,
	},
	[Host.BENCHER_CLOUD]: {
		operation: Operation.BILLING,
		header: {
			title: "Billing",
			path: (pathname) => {
				return parentPath(pathname);
			},
		},
		host: Host.BENCHER_CLOUD,
	},
};

export default billingConfig;
