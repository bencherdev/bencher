import { Resource } from "./types";
import projectsConfig from "./projects";
import testbedsConfig from "./testbeds";
import branchesConfig from "./branches";
import reportsConfig from "./reports";

const consoleConfig = {
  [Resource.PROJECTS]: projectsConfig,
  [Resource.REPORTS]: reportsConfig,
  [Resource.BRANCHES]: branchesConfig,
  [Resource.TESTBEDS]: testbedsConfig,
};

export default consoleConfig;
