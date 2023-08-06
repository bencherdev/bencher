import organizationsConfig from "./resource/organizations";

export enum Resource {
	ORGANIZATIONS,
	MEMBERS,
	BILLING,
	PROJECTS,
	REPORTS,
	METRIC_KINDS,
	BRANCHES,
	TESTBEDS,
	BENCHMARKS,
	THRESHOLDS,
	ALERTS,
	USERS,
	TOKENS,
	HELP,
}

export enum Operation {
	LIST,
	ADD,
	VIEW,
	EDIT,
	DELETE,
	PERF,
	BILLING,
	HELP,
}

const consoleConfig = {
	[Resource.ORGANIZATIONS]: organizationsConfig,
};

export default consoleConfig;
