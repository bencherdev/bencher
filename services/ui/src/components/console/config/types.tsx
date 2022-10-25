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

export enum PerKind {
  LATENCY = "latency",
  THROUGHPUT = "throughput",
  COMPUTE = "compute",
  MEMORY = "memory",
  STORAGE = "storage",
}

export const isPerfKind = (kind: string) =>
  kind === PerKind.LATENCY ||
  kind === PerKind.THROUGHPUT ||
  kind === PerKind.COMPUTE ||
  kind === PerKind.MEMORY ||
  kind === PerKind.STORAGE;

export enum PerfTab {
  BRANCHES = "branches",
  TESTBEDS = "testbeds",
  BENCHMARKS = "benchmarks",
}

export const isPerfTab = (tab: string) =>
  tab === PerfTab.BRANCHES ||
  tab === PerfTab.TESTBEDS ||
  tab === PerfTab.BENCHMARKS;
