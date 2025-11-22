// import node from "@astrojs/node";
// import netlify from "@astrojs/netlify";
import mdx from "@astrojs/mdx";
import partytown from "@astrojs/partytown";
import sitemap from "@astrojs/sitemap";
import solidJs from "@astrojs/solid-js";
import sentry from "@sentry/astro";
import expressiveCode from "astro-expressive-code";
import { defineConfig, envField } from "astro/config";
import { fromHtmlIsomorphic } from "hast-util-from-html-isomorphic";
import rehypeAutolinkHeadings from "rehype-autolink-headings";
import rehypeSlug from "rehype-slug";
import wasmPack from "vite-plugin-wasm-pack";

const CLIENT = "client";
const SERVER = "server";

const PUBLIC = "public";
const SECRET = "secret";

const BENCHER_CHAT_URL = "https://discord.gg/yGEsdUh7R4";
const BENCHER_CALENDLY_URL = "https://calendly.com/bencher/demo";
const BENCHER_GITHUB_URL = "https://github.com/bencherdev/bencher";

// https://astro.build/config
export default defineConfig({
	// https://docs.astro.build/en/reference/configuration-reference/#site
	site: "https://bencher.dev",
	output: "static",
	// This is needed for WASM
	// https://docs.astro.build/en/reference/configuration-reference/#buildassets
	// https://github.com/withastro/astro/issues/5745
	// https://github.com/nshen/vite-plugin-wasm-pack/blob/5e626b9d387b9e9df87712479df2eb5110af02f7/src/index.ts#L186
	build: {
		assets: "assets",
	},
	// DO NOT REMOVE OR MODIFY: This line is used by adapter.js
	adapter: undefined,
	// https://docs.astro.build/en/guides/environment-variables/#type-safe-environment-variables
	env: {
		schema: {
			// Set whether running as Bencher Cloud or Bencher Self-Hosted
			IS_BENCHER_CLOUD: envField.boolean({
				context: CLIENT,
				access: PUBLIC,
				optional: true,
				default: false,
			}),
			// https://support.google.com/analytics/answer/12270356?hl=en
			GOOGLE_ANALYTICS_ID: envField.string({
				context: CLIENT,
				access: PUBLIC,
				optional: true,
			}),
			// https://developers.google.com/recaptcha/docs/v3
			GOOGLE_RECAPTCHA_SITE_KEY: envField.string({
				context: CLIENT,
				access: PUBLIC,
				optional: true,
			}),
			// These values are marked as `secret` because they need to be able to be set by Bencher Self-Hosted users.
			// However, they aren't really secrets in the normal sense of the term.
			// Marking an Astro environment variable as `secret` is the only way to not have it get bundled-in at build time.
			// That is, we need these values to be set and validated at runtime.
			// https://docs.astro.build/en/guides/environment-variables/#variable-types
			BENCHER_API_URL: envField.string({
				context: SERVER,
				access: SECRET,
			}),
			// https://docs.docker.com/desktop/networking/#use-cases-and-workarounds-for-all-platforms
			INTERNAL_API_URL: envField.string({
				context: SERVER,
				access: SECRET,
				optional: true,
			}),
			// https://docs.github.com/en/apps/oauth-apps/building-oauth-apps/authenticating-to-the-rest-api-with-an-oauth-app
			OAUTH_GITHUB: envField.string({
				context: SERVER,
				access: SECRET,
				optional: true,
			}),
			// https://developers.google.com/identity/protocols/oauth2
			OAUTH_GOOGLE: envField.string({
				context: SERVER,
				access: SECRET,
				optional: true,
			}),
		},
	},
	// Do not use any trailing slashes in the paths below
	redirects: {
		"/chat": BENCHER_CHAT_URL,
		"/demo": BENCHER_CALENDLY_URL,
		"/repo": BENCHER_GITHUB_URL,
		// Redirects for old URLs
		"/docs/how-to/quick-start": "/docs/tutorial/quick-start",
		"/docs/how-to/branch-selection": "/docs/explanation/branch-selection",
		// Docs
		"/docs/[lang]": "/[lang]/docs",
		// Tutorial
		"/docs/[lang]/tutorial": "/[lang]/docs/tutorial",
		"/docs/[lang]/tutorial/quick-start": "/[lang]/docs/tutorial/quick-start",
		"/docs/[lang]/tutorial/[slug]": "/[lang]/docs/tutorial/[slug]",
		// How To
		"/docs/[lang]/how-to": "/[lang]/docs/how-to",
		"/docs/[lang]/how-to/[slug]": "/[lang]/docs/how-to/[slug]",
		// Explanation
		"/docs/[lang]/explanation": "/[lang]/docs/explanation",
		"/docs/[lang]/explanation/[slug]": "/[lang]/docs/explanation/[slug]",
		// Reference
		"/docs/[lang]/reference": "/[lang]/docs/reference",
		"/docs/[lang]/reference/api": "/[lang]/docs/reference/api",
		"/docs/[lang]/reference/architecture":
			"/[lang]/docs/reference/architecture",
		"/docs/[lang]/reference/[slug]": "/[lang]/docs/reference/[slug]",
		// Learn
		"/learn/[lang]": "/[lang]/learn",
		"/learn/[lang]/benchmarking": "/[lang]/learn/benchmarking",
		"/learn/[lang]/benchmarking/rust": "/[lang]/learn/benchmarking/rust",
		"/learn/[lang]/benchmarking/rust/[slug]":
			"/[lang]/learn/benchmarking/rust/[slug]",
	},
	// https://docs.astro.build/en/guides/internationalization/
	i18n: {
		defaultLocale: "en",
		locales: ["de", "en", "es", "fr", "ja", "ko", "pt", "ru", "zh"],
		routing: {
			prefixDefaultLocale: false,
		},
	},
	integrations: [
		// https://docs.astro.build/en/guides/integrations-guide/sitemap
		sitemap({
			// https://docs.astro.build/en/guides/integrations-guide/sitemap/#i18n
			i18n: {
				defaultLocale: "en",
				locales: {
					de: "de-DE",
					en: "en-US",
					// The `defaultLocale` value must present in `locales` keys
					es: "es-ES",
					fr: "fr-FR",
					ja: "ja-JP",
					ko: "ko-KR",
					pt: "pt-BR",
					ru: "ru-RU",
					zh: "zh-CN",
				},
			},
			// https://docs.astro.build/en/guides/integrations-guide/sitemap/#filter
			filter: (page) =>
				!(
					page.includes("bencher.dev/console") ||
					page.includes("bencher.dev/chat") ||
					page.includes("bencher.dev/demo") ||
					page.includes("bencher.dev/repo")
				),
		}),
		// Expressive Code must be before MDX
		// https://github.com/expressive-code/expressive-code/blob/main/packages/astro-expressive-code/README.md
		expressiveCode(),
		// https://docs.astro.build/en/guides/integrations-guide/mdx
		mdx(),
		// https://docs.astro.build/en/guides/integrations-guide/partytown
		partytown({
			config: {
				// https://www.kevinzunigacuellar.com/blog/google-analytics-in-astro/
				// https://partytown.builder.io/google-tag-manager#forward-events
				forward: ["dataLayer.push"],
			},
		}),
		// https://docs.astro.build/en/guides/integrations-guide/solid-js/
		solidJs({
			// https://docs.astro.build/en/guides/integrations-guide/solid-js/#devtools
			devtools: true,
		}),
		// https://docs.sentry.io/platforms/javascript/guides/astro/
		// Note that these environment variables cannot be set with `env.schema`:
		// https://docs.astro.build/en/guides/environment-variables/#limitations
		sentry({
			dsn: process.env.PUBLIC_SENTRY_DSN,
			sourceMapsUploadOptions: {
				enabled: process.env.SENTRY_UPLOAD === "true",
				org: "bencher",
				project: "bencher-console",
				authToken: process.env.SENTRY_AUTH_TOKEN,
			},
		}),
	],
	vite: {
		assetsInclude: ["**/*.sh", "**/*.ps1"],
		plugins: [wasmPack("../../lib/bencher_valid")],
	},
	markdown: {
		rehypePlugins: [
			rehypeSlug,
			[
				rehypeAutolinkHeadings,
				{
					behavior: "append",
					properties: {
						style: "padding-left: 0.3em; color: #fdb07e;",
						"aria-label": "Link to section",
					},
					content: fromHtmlIsomorphic(
						'<small><i class="fas fa-link" /></small>',
						{
							fragment: true,
						},
					),
				},
			],
		],
	},
});
