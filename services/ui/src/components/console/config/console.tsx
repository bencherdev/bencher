import { Resource } from "./types";
import organizationsConfig from "./resources/organizations";
import projectsConfig from "./resources/projects";
import testbedsConfig from "./resources/testbeds";
import branchesConfig from "./resources/branches";
import reportsConfig from "./resources/reports";
import thresholdsConfig from "./resources/thresholds";
import alertsConfig from "./resources/alerts";
import membersConfig from "./resources/members";
import metricKindsConfig from "./resources/metric_kinds";
import usersConfig from "./resources/users";

const consoleConfig = {
  [Resource.ORGANIZATIONS]: organizationsConfig,
  [Resource.MEMBERS]: membersConfig,
  [Resource.PROJECTS]: projectsConfig,
  [Resource.REPORTS]: reportsConfig,
  [Resource.BRANCHES]: branchesConfig,
  [Resource.TESTBEDS]: testbedsConfig,
  [Resource.METRIC_KINDS]: metricKindsConfig,
  [Resource.THRESHOLDS]: thresholdsConfig,
  [Resource.ALERTS]: alertsConfig,
  [Resource.USERS]: usersConfig,
};

export default consoleConfig;
