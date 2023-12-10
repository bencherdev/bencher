import mdx from "@astrojs/mdx";
import partytown from "@astrojs/partytown";
import sitemap from "@astrojs/sitemap";
import solidJs from "@astrojs/solid-js";
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
	integrations: [
		// https://docs.astro.build/en/guides/integrations-guide/sitemap
		sitemap({
			// https://docs.astro.build/en/guides/integrations-guide/sitemap/#filter
			filter: (page) =>
				!(
					page.includes("bencher.dev/console") ||
					page.includes("bencher.dev/chat") ||
					page.includes("bencher.dev/demo") ||
					page.includes("bencher.dev/repo")
				),
		}),
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
		solidJs(),
	],
	vite: {
		plugins: [wasmPack("../../lib/bencher_valid")],
	},
	markdown: {
		rehypePlugins: [
			rehypeSlug,
			[
				rehypeAutolinkHeadings,
				{
					behavior: "append",
					properties: { style: "padding-left: 0.3em; color: #fdb07e;" },
					content: fromHtmlIsomorphic(
						'<small><i class="fas fa-link" aria-hidden="true" /></small>',
						{ fragment: true },
					),
				},
			],
		],
	},
});
