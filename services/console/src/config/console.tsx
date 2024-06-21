import billingConfig from "./organization/billing";
import membersConfig from "./organization/members";
import organizationsConfig from "./organization/organizations";
import alertsConfig from "./project/alerts";
import benchmarksConfig from "./project/benchmarks";
import branchesConfig from "./project/branches";
import measuresConfig from "./project/measures";
import metricsConfig from "./project/metrics";
import projectsConfig from "./project/projects";
import reportsConfig from "./project/reports";
import testbedsConfig from "./project/testbeds";
import thresholdsConfig from "./project/thresholds";
import { BencherResource } from "./types";
import tokensConfig from "./user/tokens";
import usersConfig from "./user/users";

const consoleConfig = {
	// Organization
	[BencherResource.ORGANIZATIONS]: organizationsConfig,
	[BencherResource.MEMBERS]: membersConfig,
	[BencherResource.BILLING]: billingConfig,
	// Project
	[BencherResource.PROJECTS]: projectsConfig,
	[BencherResource.REPORTS]: reportsConfig,
	[BencherResource.BRANCHES]: branchesConfig,
	[BencherResource.TESTBEDS]: testbedsConfig,
	[BencherResource.BENCHMARKS]: benchmarksConfig,
	[BencherResource.MEASURES]: measuresConfig,
	[BencherResource.METRICS]: metricsConfig,
	[BencherResource.THRESHOLDS]: thresholdsConfig,
	[BencherResource.ALERTS]: alertsConfig,
	// User
	[BencherResource.USERS]: usersConfig,
	[BencherResource.TOKENS]: tokensConfig,
};

export default consoleConfig;
