import { parentPath } from "../util";

export enum Host {
	SELF_HOSTED = "self_hosted",
	BENCHER_CLOUD = "bencher_cloud",
}

const billingConfig = {
	[Host.SELF_HOSTED]: {
		header: {
			title: "Self-Hosted Billing",
			path: (pathname: string) => `${parentPath(pathname)}/projects`,
		},
		host: Host.SELF_HOSTED,
	},
	[Host.BENCHER_CLOUD]: {
		header: {
			title: "Billing",
			path: (pathname: string) => `${parentPath(pathname)}/projects`,
		},
		host: Host.BENCHER_CLOUD,
	},
};

export default billingConfig;
