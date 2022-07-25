import { defineConfig } from "vite";
import solidPlugin from "vite-plugin-solid";
import wasmPack from "vite-plugin-wasm-pack";

export default defineConfig({
  plugins: [solidPlugin(), wasmPack("../lib/bencher_json")],
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
