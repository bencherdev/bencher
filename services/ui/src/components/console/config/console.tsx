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
import tokensConfig from "./resources/tokens";
import benchmarksConfig from "./resources/benchmarks";
import billingConfig from "./resources/billing";
import helpConfig from "./resources/help";

const consoleConfig = {
	[Resource.ORGANIZATIONS]: organizationsConfig,
	[Resource.MEMBERS]: membersConfig,
	[Resource.BILLING]: billingConfig,
	[Resource.PROJECTS]: projectsConfig,
	[Resource.REPORTS]: reportsConfig,
	[Resource.METRIC_KINDS]: metricKindsConfig,
	[Resource.BRANCHES]: branchesConfig,
	[Resource.TESTBEDS]: testbedsConfig,
	[Resource.BENCHMARKS]: benchmarksConfig,
	[Resource.THRESHOLDS]: thresholdsConfig,
	[Resource.ALERTS]: alertsConfig,
	[Resource.USERS]: usersConfig,
	[Resource.TOKENS]: tokensConfig,
	[Resource.HELP]: helpConfig,
};

export default consoleConfig;
