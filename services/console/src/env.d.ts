/// <reference path="../.astro/types.d.ts" />

interface ImportMetaEnv {
	// https://docs.astro.build/en/guides/environment-variables/#default-environment-variables
	readonly MODE: string;
	// https://docs.astro.build/en/guides/environment-variables
	readonly PUBLIC_GOOGLE_ANALYTICS_ID?: string;
	readonly PUBLIC_IS_BENCHER_CLOUD?: string;
	// https://docs.astro.build/en/guides/integrations-guide/node/#runtime-environment-variables
	readonly BENCHER_API_URL: string;
}

interface ImportMeta {
	readonly env: ImportMetaEnv;
}
