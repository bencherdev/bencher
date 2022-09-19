import { Resource } from "./types";
import organizationsConfig from "./organizations";
import projectsConfig from "./projects";
import testbedsConfig from "./testbeds";
import branchesConfig from "./branches";
import reportsConfig from "./reports";
import thresholdsConfig from "./thresholds";

const consoleConfig = {
  [Resource.ORGANIZATIONS]: organizationsConfig,
  [Resource.PROJECTS]: projectsConfig,
  [Resource.REPORTS]: reportsConfig,
  [Resource.BRANCHES]: branchesConfig,
  [Resource.TESTBEDS]: testbedsConfig,
  [Resource.THRESHOLDS]: thresholdsConfig,
};

export default consoleConfig;
