// Plain JavaScript, since `astro.config.mjs` loads it before anything compiles.
import { readdir, readFile, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

// The same placeholder `src/next/preloads.ts` holds; the speed ceilings fail
// on modules a page finds late, so a mismatch cannot go unnoticed.
const PLACEHOLDER = "@@BENCHER_NEXT_PRELOADS@@";

/**
 * Write the new console's client module graph into the server bundle, so its
 * document names every module a page loads and the browser fetches them in
 * one round instead of finding each import only once its parent arrives.
 *
 * @returns {import("astro").AstroIntegration}
 */
const nextPreloads = () => {
	/** @type {{ entry: string[], pages: Record<string, string[]> } | undefined} */
	let preloads;
	/** @type {URL | undefined} */
	let serverDir;

	const collect = {
		name: "bencher-next-preloads",
		/**
		 * @this {{ environment: { name: string } }}
		 * @param {unknown} _options
		 * @param {Record<string, { type: string, fileName: string, name?: string, isEntry?: boolean, isDynamicEntry?: boolean, facadeModuleId?: string | null, imports?: string[] }>} bundle
		 */
		generateBundle(_options, bundle) {
			if (this.environment.name !== "client") {
				return;
			}
			const chunks = new Map(
				Object.values(bundle)
					.filter((output) => output.type === "chunk")
					.map((chunk) => [chunk.fileName, chunk]),
			);
			/** @type {(fileName: string, seen?: Set<string>) => Set<string>} */
			const closure = (fileName, seen = new Set()) => {
				if (!seen.has(fileName)) {
					seen.add(fileName);
					for (const imported of chunks.get(fileName)?.imports ?? []) {
						closure(imported, seen);
					}
				}
				return seen;
			};
			/** @type {Set<string>} */
			const entry = new Set();
			for (const chunk of chunks.values()) {
				const isNextEntry =
					chunk.isEntry && chunk.facadeModuleId?.includes("/src/pages/next/");
				// Astro's page script, which every page loads.
				const isPageScript = chunk.isEntry && chunk.name === "page";
				if (isNextEntry || isPageScript) {
					closure(chunk.fileName, entry);
				}
			}
			/** @type {Record<string, string[]>} */
			const pages = {};
			for (const chunk of chunks.values()) {
				const page = chunk.facadeModuleId?.match(
					/\/src\/next\/pages\/(\w+)\.tsx$/,
				)?.[1];
				if (chunk.isDynamicEntry && page) {
					pages[page] = [...closure(chunk.fileName)]
						.filter((file) => !entry.has(file))
						.map((file) => `/${file}`);
				}
			}
			preloads = { entry: [...entry].map((file) => `/${file}`), pages };
		},
	};

	return {
		name: "bencher-next-preloads",
		hooks: {
			"astro:config:setup": ({ updateConfig }) => {
				updateConfig({ vite: { plugins: [collect] } });
			},
			"astro:config:done": ({ config }) => {
				serverDir = config.build.server;
			},
			"astro:build:done": async () => {
				if (!(preloads && serverDir)) {
					return;
				}
				const json = JSON.stringify(preloads);
				const quoted = new RegExp(`["']${PLACEHOLDER}["']`, "g");
				for (const file of await scripts(fileURLToPath(serverDir))) {
					const code = await readFile(file, "utf8");
					if (code.includes(PLACEHOLDER)) {
						await writeFile(
							file,
							code.replace(quoted, () => json),
						);
					}
				}
			},
		},
	};
};

export default nextPreloads;

/** @param {string} dir */
const scripts = async (dir) =>
	(await readdir(dir, { recursive: true, withFileTypes: true }))
		.filter((entry) => entry.isFile() && /\.m?js$/.test(entry.name))
		.map((entry) => join(entry.parentPath, entry.name));
