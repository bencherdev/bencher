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

export const resourceSingular = (resource: Resource) => {
	switch (resource) {
		case Resource.ORGANIZATIONS:
			return "organization";
		case Resource.MEMBERS:
			return "member";
		case Resource.BILLING:
			return "billing";
		case Resource.PROJECTS:
			return "project";
		case Resource.REPORTS:
			return "report";
		case Resource.METRIC_KINDS:
			return "metric kind";
		case Resource.BRANCHES:
			return "branch";
		case Resource.TESTBEDS:
			return "testbed";
		case Resource.BENCHMARKS:
			return "benchmark";
		case Resource.THRESHOLDS:
			return "threshold";
		case Resource.ALERTS:
			return "alert";
		case Resource.USERS:
			return "user";
		case Resource.TOKENS:
			return "token";
		case Resource.HELP:
			return "help";
	}
};

export const resourcePlural = (resource: Resource) => {
	switch (resource) {
		case Resource.ORGANIZATIONS:
			return "organizations";
		case Resource.MEMBERS:
			return "members";
		case Resource.BILLING:
			return "billing";
		case Resource.PROJECTS:
			return "projects";
		case Resource.REPORTS:
			return "reports";
		case Resource.METRIC_KINDS:
			return "metric kinds";
		case Resource.BRANCHES:
			return "branches";
		case Resource.TESTBEDS:
			return "testbeds";
		case Resource.BENCHMARKS:
			return "benchmarks";
		case Resource.THRESHOLDS:
			return "thresholds";
		case Resource.ALERTS:
			return "alerts";
		case Resource.USERS:
			return "users";
		case Resource.TOKENS:
			return "tokens";
		case Resource.HELP:
			return "help";
	}
};

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
