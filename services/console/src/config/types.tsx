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
}

export enum Button {
	ADD,
	INVITE,
	EDIT,
	STATUS,
	PERF,
	REFRESH,
	BACK,
}

export enum ActionButton {
	DELETE,
}

export enum Row {
	TEXT,
	DATE_TIME,
	BOOL,
	SELECT,
	NESTED_TEXT,
}

export enum Card {
	FIELD,
	TABLE,
	NESTED_FIELD,
}

export enum Display {
	RAW,
	SWITCH,
	SELECT,
}

export enum PerfTab {
	REPORTS = "reports",
	BRANCHES = "branches",
	TESTBEDS = "testbeds",
	BENCHMARKS = "benchmarks",
}

export const isPerfTab = (tab: undefined | string) =>
	tab === PerfTab.REPORTS ||
	tab === PerfTab.BRANCHES ||
	tab === PerfTab.TESTBEDS ||
	tab === PerfTab.BENCHMARKS;

export enum PerfRange {
	DATE_TIME = "date_time",
	VERSION = "version",
}

export const isPerfRange = (range: undefined | string) =>
	range === PerfRange.DATE_TIME || range === PerfRange.VERSION;
