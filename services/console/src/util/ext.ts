import swagger from "../content/api/swagger.json";
import { apiHost } from "./http";

export const BENCHER_SITE_URL = "https://bencher.dev";

export const BENCHER_CLOUD_API_URL: string = "https://api.bencher.dev";

export const BENCHER_GITHUB_URL: string =
	"https://github.com/bencherdev/bencher";

export const BENCHER_CALENDLY_URL: string = "https://calendly.com/bencher/demo";

export const BENCHER_CHAT_URL: string = "https://discord.gg/yGEsdUh7R4";

export const BENCHER_LOGO_URL: string =
	"https://s3.amazonaws.com/public.bencher.dev/bencher_navbar.png";

export const BENCHER_VERSION = `${swagger?.info?.version}`;

export const SWAGGER = swagger;
export const BENCHER_CLOUD = "Bencher Cloud";
export const BENCHER_SELF_HOSTED = "Bencher Self-Hosted";

export const swaggerSpec = (apiUrl: string) => {
	const url = apiHost(apiUrl);

	const swagger = SWAGGER;
	// https://swagger.io/docs/specification/api-host-and-base-path/
	swagger.servers = [];
	if (!isBencherCloud(url)) {
		swagger.servers.push({
			url: url,
			description: BENCHER_SELF_HOSTED,
		});
	}
	swagger.servers.push({
		url: BENCHER_CLOUD_API_URL,
		description: BENCHER_CLOUD,
	});

	return [url, swagger];
};

// Change this value to test billing in development mode
const TEST_BENCHER_BILLING: boolean = true;

export const isBencherCloud = (apiUrl: string) => {
	const mode = import.meta.env.MODE;
	switch (mode) {
		case "development":
			return TEST_BENCHER_BILLING;
		case "production":
			return apiUrl === BENCHER_CLOUD_API_URL;
		default:
			console.error("Invalid mode: ", mode);
			return false;
	}
};
