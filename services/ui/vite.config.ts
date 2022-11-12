import { defineConfig } from "vite";
import solid from "vite-plugin-solid";
import mdx from "@mdx-js/rollup";
import remarkGfm from "remark-gfm";

export default defineConfig({
  plugins: [mdx({ jsxImportSource: "solid-jsx", remarkPlugins: [remarkGfm] }), solid()],
  build: {
    target: "esnext",
  },
  server: {
    host: true,
    port: 3000,
    hmr: {
      port: 3001,
    },
  },
});
