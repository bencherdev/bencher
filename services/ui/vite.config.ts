import { defineConfig } from "vite";
import solidPlugin from "vite-plugin-solid";

export default defineConfig({
  plugins: [solidPlugin()],
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
