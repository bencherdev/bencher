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
			return "an organization";
		case Resource.MEMBERS:
			return "a member";
		case Resource.BILLING:
			return "your billing";
		case Resource.PROJECTS:
			return "a project";
		case Resource.REPORTS:
			return "a report";
		case Resource.METRIC_KINDS:
			return "a metric kind";
		case Resource.BRANCHES:
			return "a branch";
		case Resource.TESTBEDS:
			return "a testbed";
		case Resource.BENCHMARKS:
			return "a benchmark";
		case Resource.THRESHOLDS:
			return "a threshold";
		case Resource.ALERTS:
			return "an alert";
		case Resource.USERS:
			return "a user";
		case Resource.TOKENS:
			return "a token";
		case Resource.HELP:
			return "your help";
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
