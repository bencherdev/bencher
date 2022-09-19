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
  REFRESH,
  BACK,
}

export enum Resource {
  ORGANIZATIONS,
  PROJECTS,
  REPORTS,
  BRANCHES,
  TESTBEDS,
  THRESHOLDS,
  CONNECTIONS,
  PROJECT_SETTINGS,
  USER_ACCOUNT,
  USER_SETTINGS,
}

export enum Field {
  FIXED,
  INPUT,
  TEXTAREA,
  CHECKBOX,
  SWITCH,
  SELECT,
}

export enum Row {
  TEXT,
  BOOL,
}

export enum Card {
  FIELD,
  TABLE,
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
