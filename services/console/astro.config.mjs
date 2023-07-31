import { defineConfig } from 'astro/config';
import solidJs from "@astrojs/solid-js";
import sitemap from "@astrojs/sitemap";
import mdx from "@astrojs/mdx";
import wasmPack from "vite-plugin-wasm-pack";

// https://astro.build/config
export default defineConfig({
  output: "hybrid",
  integrations: [sitemap(), mdx(), solidJs()],
  vite: {
    plugins: [wasmPack("../../lib/bencher_valid")],
  },
});