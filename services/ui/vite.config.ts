import { defineConfig } from "vite";
import solidPlugin from "vite-plugin-solid";
import remarkGfm from "remark-gfm";
import mdx from "@mdx-js/rollup"

export default defineConfig({
  plugins: [mdx({ jsxImportSource: "solid-jsx", remarkPlugins: [remarkGfm] }), solidPlugin()],
  build: {
    target: "esnext",
    polyfillDynamicImport: false,
  },
  server: {
    host: true,
    hmr: {
      port: 3001,
    },
  },
});
