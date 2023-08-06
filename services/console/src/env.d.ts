/// <reference path="../.astro/types.d.ts" />
/// <reference types="astro/client" />

interface ImportMetaEnv {
	readonly PUBLIC_BENCHER_API_URL: string;
	readonly PUBLIC_GOOGLE_ANALYTICS_ID?: string;
}

interface ImportMeta {
	readonly env: ImportMetaEnv;
}
