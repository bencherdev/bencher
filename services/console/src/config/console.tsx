import { BencherResource } from "./types";
import membersConfig from "./organization/members";
import organizationsConfig from "./organization/organizations";
import projectsConfig from "./project/projects";
import tokensConfig from "./user/tokens";
import usersConfig from "./user/users";
import billingConfig from "./organization/billing";
import reportsConfig from "./project/reports";
import metricKindsConfig from "./project/metric_kinds";
import branchesConfig from "./project/branches";
import testbedsConfig from "./project/testbeds";
import benchmarksConfig from "./project/benchmarks";
import thresholdsConfig from "./project/thresholds";
import alertsConfig from "./project/alerts";

const consoleConfig = {
	// Organization
	[BencherResource.ORGANIZATIONS]: organizationsConfig,
	[BencherResource.MEMBERS]: membersConfig,
	[BencherResource.BILLING]: billingConfig,
	// Project
	[BencherResource.PROJECTS]: projectsConfig,
	[BencherResource.REPORTS]: reportsConfig,
	[BencherResource.METRIC_KINDS]: metricKindsConfig,
	[BencherResource.BRANCHES]: branchesConfig,
	[BencherResource.TESTBEDS]: testbedsConfig,
	[BencherResource.BENCHMARKS]: benchmarksConfig,
	[BencherResource.THRESHOLDS]: thresholdsConfig,
	[BencherResource.ALERTS]: alertsConfig,
	// User
	[BencherResource.USERS]: usersConfig,
	[BencherResource.TOKENS]: tokensConfig,
};

export default consoleConfig;
