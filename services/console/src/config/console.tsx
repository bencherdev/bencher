import { Resource } from "./types";
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

const consoleConfig = {
	// Organization
	[Resource.ORGANIZATIONS]: organizationsConfig,
	[Resource.MEMBERS]: membersConfig,
	[Resource.BILLING]: billingConfig,
	// Project
	[Resource.PROJECTS]: projectsConfig,
	[Resource.REPORTS]: reportsConfig,
	[Resource.METRIC_KINDS]: metricKindsConfig,
	[Resource.BRANCHES]: branchesConfig,
	[Resource.TESTBEDS]: testbedsConfig,
	[Resource.BENCHMARKS]: benchmarksConfig,
	// User
	[Resource.USERS]: usersConfig,
	[Resource.TOKENS]: tokensConfig,
};

export default consoleConfig;
