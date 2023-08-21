import { defineConfig } from "astro/config";
import solidJs from "@astrojs/solid-js";
import sitemap from "@astrojs/sitemap";
import mdx from "@astrojs/mdx";
import wasmPack from "vite-plugin-wasm-pack";
import remarkGfm from "remark-gfm";

import partytown from "@astrojs/partytown";

// https://astro.build/config
export default defineConfig({
	// https://docs.astro.build/en/reference/configuration-reference/#site
	site: "https://bencher.dev",
	output: "hybrid",
	integrations: [sitemap(), mdx(), solidJs(), partytown()],
	vite: {
		plugins: [wasmPack("../../lib/bencher_valid")],
	},
	markdown: {
		remarkPlugins: [remarkGfm],
	},
	experimental: {
		// https://docs.astro.build/en/guides/view-transitions
		viewTransitions: false,
	},
});
