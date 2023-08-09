import organizationsConfig from "./resource/organizations";
import { Resource } from "./types";
import tokensConfig from "./user/tokens";
import usersConfig from "./user/users";

const consoleConfig = {
	[Resource.ORGANIZATIONS]: organizationsConfig,
	[Resource.USERS]: usersConfig,
	[Resource.TOKENS]: tokensConfig,
};

export default consoleConfig;
