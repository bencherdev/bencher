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

export const is_perf_tab = (tab: string) =>
	tab === PerfTab.REPORTS ||
	tab === PerfTab.BRANCHES ||
	tab === PerfTab.TESTBEDS ||
	tab === PerfTab.BENCHMARKS;

export enum Range {
	DATE_TIME = "date_time",
	VERSION = "version",
}

export const isRange = (range: string) =>
	range === Range.DATE_TIME || range === Range.VERSION;
