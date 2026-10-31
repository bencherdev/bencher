import { fileURLToPath } from "node:url";
import { defineConfig } from "vite";
import solid from "vite-plugin-solid";
import { viteSingleFile } from "vite-plugin-singlefile";

// The MCP App's page in one file, committed beside the MCP server that embeds and serves it.
export default defineConfig({
	root: fileURLToPath(new URL("./src/next/mcp", import.meta.url)),
	publicDir: false,
	plugins: [solid(), viteSingleFile()],
	css: {
		postcss: {
			// The page loads Inter from its bytes, so its stylesheet asks for no font.
			plugins: [
				{
					postcssPlugin: "no-font-face",
					AtRule: {
						"font-face": (rule) => {
							rule.remove();
						},
					},
				},
			],
		},
	},
	resolve: {
		dedupe: ["solid-js", "solid-js/web", "solid-js/store"],
	},
	build: {
		outDir: fileURLToPath(new URL("../../plus/api_mcp/ui", import.meta.url)),
		emptyOutDir: false,
		rolldownOptions: {
			input: fileURLToPath(
				new URL("./src/next/mcp/perf.html", import.meta.url),
			),
		},
	},
});
