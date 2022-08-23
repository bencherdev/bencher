import { Resource } from "./types";
import projectsConfig from "./projects";
import testbedsConfig from "./testbeds";
import branchesConfig from "./branches";
import reportsConfig from "./reports";
import thresholdsConfig from "./thresholds";

const consoleConfig = {
  [Resource.PROJECTS]: projectsConfig,
  [Resource.REPORTS]: reportsConfig,
  [Resource.BRANCHES]: branchesConfig,
  [Resource.TESTBEDS]: testbedsConfig,
  [Resource.THRESHOLDS]: thresholdsConfig,
};

export default consoleConfig;
