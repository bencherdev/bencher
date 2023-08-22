import swagger from "../content/api/swagger.json";

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
