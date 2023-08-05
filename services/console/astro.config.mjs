import { defineConfig } from "astro/config";
import solidJs from "@astrojs/solid-js";
import sitemap from "@astrojs/sitemap";
import mdx from "@astrojs/mdx";
import wasmPack from "vite-plugin-wasm-pack";

// https://astro.build/config
export default defineConfig({
  // https://docs.astro.build/en/reference/configuration-reference/#site
  site: "https://bencher.dev",
  output: "hybrid",
  integrations: [sitemap(), mdx(), solidJs()],
  vite: {
    plugins: [wasmPack("../../lib/bencher_valid")],
  },
});