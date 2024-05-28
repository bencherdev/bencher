import { XAxis } from "../types/bencher";

export enum BencherResource {
	ORGANIZATIONS = "organizations",
	MEMBERS = "members",
	BILLING = "billing",
	PROJECTS = "projects",
	REPORTS = "reports",
	BRANCHES = "branches",
	TESTBEDS = "testbeds",
	BENCHMARKS = "benchmarks",
	MEASURES = "measures",
	THRESHOLDS = "thresholds",
	ALERTS = "alerts",
	USERS = "users",
	TOKENS = "tokens",
	HELP = "help",
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
		case BencherResource.BRANCHES:
			return "branch";
		case BencherResource.TESTBEDS:
			return "testbed";
		case BencherResource.BENCHMARKS:
			return "benchmark";
		case BencherResource.MEASURES:
			return "measure";
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
		case BencherResource.BRANCHES:
			return "branches";
		case BencherResource.TESTBEDS:
			return "testbeds";
		case BencherResource.BENCHMARKS:
			return "benchmarks";
		case BencherResource.MEASURES:
			return "measures";
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
	LIST = "list",
	ADD = "add",
	VIEW = "view",
	EDIT = "edit",
	DELETE = "delete",
}

export enum Button {
	ADD = "add",
	INVITE = "invite",
	EDIT = "edit",
	STATUS = "status",
	CONSOLE = "console",
	PERF = "perf",
	REFRESH = "refresh",
	BACK = "back",
	SEARCH = "search",
	DATE_TIME = "date_time",
}

export enum ActionButton {
	DELETE = "delete",
}

export enum Row {
	TEXT = "text",
	DATE_TIME = "date_time",
	BOOL = "bool",
	SELECT = "select",
	NESTED_TEXT = "nested_text",
}

export enum Card {
	FIELD = "field",
	TABLE = "table",
	NESTED_FIELD = "nested_field",
}

export enum Display {
	RAW = "raw",
	SWITCH = "switch",
	SELECT = "select",
	START_POINT = "start_point",
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

export const isXAxis = (xAxis: undefined | string) => {
	switch (xAxis) {
		case XAxis.DateTime:
		case XAxis.Version:
			return true;
		default:
			return false;
	}
};

export const embedHeight = 780;
