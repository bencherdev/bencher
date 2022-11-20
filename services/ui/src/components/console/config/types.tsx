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

export enum Field {
  HIDDEN,
  INPUT,
  TEXTAREA,
  CHECKBOX,
  SWITCH,
  SELECT,
}

export enum Row {
  TEXT,
  BOOL,
  SELECT,
}

export enum Card {
  FIELD,
  TABLE,
}

export enum Display {
  RAW,
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

export const isMetricKind = (metric_kind: any) =>
  typeof metric_kind === "string" && metric_kind.length > 0;
