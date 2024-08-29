import mdx from "@astrojs/mdx";
// import partytown from "@astrojs/partytown";
import sitemap from "@astrojs/sitemap";
import solidJs from "@astrojs/solid-js";
import sentry from "@sentry/astro";
import expressiveCode from "astro-expressive-code";
import { defineConfig } from "astro/config";
import { fromHtmlIsomorphic } from "hast-util-from-html-isomorphic";
import rehypeAutolinkHeadings from "rehype-autolink-headings";
import rehypeSlug from "rehype-slug";
import wasmPack from "vite-plugin-wasm-pack";

// https://astro.build/config
export default defineConfig({
	// https://docs.astro.build/en/reference/configuration-reference/#site
	site: "https://bencher.dev",
	output: "hybrid",
	// DO NOT REMOVE OR MODIFY: This line is used by adapter.js
	adapter: undefined,
	// Do not use any trailing slashes in the paths below
	redirects: {
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
		// partytown({
		// 	config: {
		// 		// https://www.kevinzunigacuellar.com/blog/google-analytics-in-astro/
		// 		// https://partytown.builder.io/google-tag-manager#forward-events
		// 		forward: ["dataLayer.push"],
		// 	},
		// }),
		// https://docs.astro.build/en/guides/integrations-guide/solid-js/
		solidJs(),
		// https://docs.sentry.io/platforms/javascript/guides/astro/
		sentry({
			dsn: process.env.PUBLIC_SENTRY_DSN,
			sourceMapsUploadOptions: {
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
