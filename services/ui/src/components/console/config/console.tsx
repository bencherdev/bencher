import { Resource } from "./types";
import projectsConfig from "./projects";
import testbedsConfig from "./testbeds";
import branchesConfig from "./branches";

const consoleConfig = {
  [Resource.PROJECTS]: projectsConfig,
  [Resource.BRANCHES]: branchesConfig,
  [Resource.TESTBEDS]: testbedsConfig,
};

export default consoleConfig;
