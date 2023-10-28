export enum BencherResource {
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

export const resourceSingular = (resource: BencherResource) => {
	switch (resource) {
		case BencherResource.ORGANIZATIONS:
			return "organization";
		case BencherResource.MEMBERS:
			return "member";
		case BencherResource.BILLING:
			return "billing";
		case BencherResource.PROJECTS:
			return "project";
		case BencherResource.REPORTS:
			return "report";
		case BencherResource.METRIC_KINDS:
			return "metric kind";
		case BencherResource.BRANCHES:
			return "branch";
		case BencherResource.TESTBEDS:
			return "testbed";
		case BencherResource.BENCHMARKS:
			return "benchmark";
		case BencherResource.THRESHOLDS:
			return "threshold";
		case BencherResource.ALERTS:
			return "alert";
		case BencherResource.USERS:
			return "user";
		case BencherResource.TOKENS:
			return "token";
		case BencherResource.HELP:
			return "help";
	}
};

export const resourcePlural = (resource: BencherResource) => {
	switch (resource) {
		case BencherResource.ORGANIZATIONS:
			return "organizations";
		case BencherResource.MEMBERS:
			return "members";
		case BencherResource.BILLING:
			return "billing";
		case BencherResource.PROJECTS:
			return "projects";
		case BencherResource.REPORTS:
			return "reports";
		case BencherResource.METRIC_KINDS:
			return "metric kinds";
		case BencherResource.BRANCHES:
			return "branches";
		case BencherResource.TESTBEDS:
			return "testbeds";
		case BencherResource.BENCHMARKS:
			return "benchmarks";
		case BencherResource.THRESHOLDS:
			return "thresholds";
		case BencherResource.ALERTS:
			return "alerts";
		case BencherResource.USERS:
			return "users";
		case BencherResource.TOKENS:
			return "tokens";
		case BencherResource.HELP:
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

export const embedHeight = 720;
