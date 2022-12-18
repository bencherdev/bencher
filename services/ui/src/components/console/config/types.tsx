export enum Operation {
  LIST,
  ADD,
  VIEW,
  EDIT,
  DELETE,
  PERF,
}

export enum Button {
  ADD,
  INVITE,
  REFRESH,
  BACK,
}

export enum Resource {
  ORGANIZATIONS,
  ORGANIZATION_SETTINGS,
  MEMBERS,
  PROJECTS,
  PROJECT_SETTINGS,
  REPORTS,
  BRANCHES,
  TESTBEDS,
  METRIC_KINDS,
  THRESHOLDS,
  ALERTS,
  CONNECTIONS,
  USER_ACCOUNT,
  USER_SETTINGS,
}

export enum FieldKind {
  HIDDEN,
  INPUT,
  CHECKBOX,
  SWITCH,
  SELECT,
  TABLE,
}

export enum Row {
  TEXT,
  BOOL,
  SELECT,
  FOREIGN,
}

export enum Card {
  FIELD,
  TABLE,
}

export enum Display {
  RAW,
  SWITCH,
  SELECT,
}

export enum PerfTab {
  BRANCHES = "branches",
  TESTBEDS = "testbeds",
  BENCHMARKS = "benchmarks",
}

export const isPerfTab = (tab: string) =>
  tab === PerfTab.BRANCHES ||
  tab === PerfTab.TESTBEDS ||
  tab === PerfTab.BENCHMARKS;
