import { defineConfig } from "vite";
import solidPlugin from "vite-plugin-solid";
import mdx from "@mdx-js/rollup"

export default defineConfig({
  plugins: [solidPlugin(), mdx({ jsxImportSource: "solid-jsx"})],
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
