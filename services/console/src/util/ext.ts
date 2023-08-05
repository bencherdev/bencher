import swagger from "../content/swagger/swagger.json";

export const BENCHER_CLOUD_API_URL: string = "https://api.bencher.dev";

export const BENCHER_GITHUB_URL: string =
	"https://github.com/bencherdev/bencher";

export const BENCHER_CALENDLY_URL: string = "https://calendly.com/bencher/demo";

export const BENCHER_CHAT_URL: string = "https://discord.gg/yGEsdUh7R4";

export const BENCHER_LOGO_URL: string =
	"https://s3.amazonaws.com/public.bencher.dev/bencher_navbar.png";

export const BENCHER_USER_KEY: string = "BENCHER_USER";

export const BENCHER_TITLE = "Bencher - Continuous Benchmarking";

export const BENCHER_VERSION = `${swagger?.info?.version}`;

// Either supply `VITE_BENCHER_API_URL` at build time,
// or default to the current protocol and hostname at port `61016`.
// If another endpoint is required, then the UI will need to be re-bundled.
export const BENCHER_API_URL: () => string = () => {
	const api_url = import.meta.env.VITE_BENCHER_API_URL;
	if (api_url) {
		return api_url;
	} else {
		const location = window.location;
		return `${location.protocol}//${location.hostname}:61016`;
	}
};