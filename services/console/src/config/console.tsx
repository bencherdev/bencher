import organizationsConfig from "./resource/organizations";
import { Resource } from "./types";
import tokensConfig from "./user/tokens";

const consoleConfig = {
	[Resource.ORGANIZATIONS]: organizationsConfig,
	[Resource.TOKENS]: tokensConfig,
};

export default consoleConfig;
