import swagger from "../../../api/openapi.json";

export const BENCHER_SITE_URL = "https://bencher.dev";
export const BENCHER_CLOUD_API_URL: string = "https://api.bencher.dev";

export const BENCHER_GITHUB_URL: string =
	"https://github.com/bencherdev/bencher";

export const BENCHER_CALENDLY_URL: string = "https://calendly.com/bencher/demo";

export const BENCHER_CHAT_URL: string = "https://discord.gg/yGEsdUh7R4";

export const BENCHER_LINKEDIN_URL: string =
	"https://www.linkedin.com/company/bencher";

export const BENCHER_REDDIT_URL: string = "https://www.reddit.com/r/bencher";

export const BENCHER_LOGO: string = "/favicon.svg";
export const BENCHER_WORDMARK_ID: string = "bencher-wordmark";
export const BENCHER_WORDMARK_INLINE_ID: string = "bencher-wordmark-inline";
export const BENCHER_WORDMARK_FOOTER_ID: string = "bencher-wordmark-footer";
export const BENCHER_WORDMARK: string = "/wordmark.svg";
export const BENCHER_WORDMARK_DARK: string = "/wordmark-dark.svg";

export const BENCHER_VERSION = `${swagger?.info?.version}`;

export const SWAGGER = swagger;
export const BENCHER_CLOUD = "Bencher Cloud";
export const BENCHER_SELF_HOSTED = "Bencher Self-Hosted";

export const isBencherCloud = (): boolean =>
	`${import.meta.env.PUBLIC_IS_BENCHER_CLOUD}` === "true";
