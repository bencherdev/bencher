import { Resource } from "./types";
import membersConfig from "./organization/members";
import organizationsConfig from "./organization/organizations";
import projectsConfig from "./project/projects";
import tokensConfig from "./user/tokens";
import usersConfig from "./user/users";

const consoleConfig = {
	[Resource.ORGANIZATIONS]: organizationsConfig,
	[Resource.MEMBERS]: membersConfig,
	[Resource.PROJECTS]: projectsConfig,
	[Resource.USERS]: usersConfig,
	[Resource.TOKENS]: tokensConfig,
};

export default consoleConfig;
