import { defineConfig } from "astro/config";
import solidJs from "@astrojs/solid-js";
import sitemap from "@astrojs/sitemap";
import mdx from "@astrojs/mdx";
import partytown from "@astrojs/partytown";
import wasmPack from "vite-plugin-wasm-pack";
import remarkGfm from "remark-gfm";

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
		remarkPlugins: [remarkGfm],
	},
});
