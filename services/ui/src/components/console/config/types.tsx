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

export enum Button {
	ADD,
	INVITE,
	REFRESH,
	BACK,
}

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

export const is_perf_tab = (tab: string) =>
	tab === PerfTab.REPORTS ||
	tab === PerfTab.BRANCHES ||
	tab === PerfTab.TESTBEDS ||
	tab === PerfTab.BENCHMARKS;

export enum Range {
	DATE_TIME = "date_time",
	VERSION = "version",
}

export const is_range = (range: string) =>
	range === Range.DATE_TIME || range === Range.VERSION;
