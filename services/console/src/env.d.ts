/// <reference path="../.astro/types.d.ts" />

interface ImportMetaEnv {
	// https://docs.astro.build/en/guides/environment-variables/#default-environment-variables
	readonly MODE: string;
	// https://docs.astro.build/en/guides/environment-variables
	readonly PUBLIC_IS_BENCHER_CLOUD?: string;
	// https://support.google.com/analytics/answer/12270356?hl=en
	readonly PUBLIC_GOOGLE_ANALYTICS_ID?: string;
	// https://docs.sentry.io/platforms/javascript/guides/astro/
	readonly PUBLIC_SENTRY_DSN?: string;
	// https://docs.astro.build/en/guides/integrations-guide/node/#runtime-environment-variables
	readonly BENCHER_API_URL: string;
	// https://docs.docker.com/desktop/networking/#use-cases-and-workarounds-for-all-platforms
	readonly INTERNAL_API_URL: string;
	// https://docs.github.com/en/apps/oauth-apps/building-oauth-apps/authenticating-to-the-rest-api-with-an-oauth-app
	readonly GITHUB_CLIENT_ID: string;
	// https://docs.sentry.io/platforms/javascript/guides/astro/
	readonly SENTRY_AUTH_TOKEN: string;
}

interface ImportMeta {
	readonly env: ImportMetaEnv;
}
