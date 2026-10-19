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
	/** @type {{ entry: string[], pages: Record<string, string[]>, parts: Record<string, string[]>, perf: { entry: string[], parts: Record<string, string[]> }, embed: { entry: string[], parts: Record<string, string[]> } } | undefined} */
	let preloads;
	/** @type {URL | undefined} */
	let serverDir;
	/** @type {Map<string, Set<string>> | undefined} */
	let loadedBefore;

	const collect = {
		name: "bencher-next-preloads",
		// A lazy import's preload list leaves out what its importer already
		// loaded, which every page's entry would otherwise carry for each route.
		configEnvironment(/** @type {string} */ name) {
			if (name !== "client") {
				return;
			}
			return {
				build: {
					modulePreload: {
						/** @type {(file: string, deps: string[], context: { hostId: string }) => string[]} */
						resolveDependencies: (file, deps, { hostId }) => {
							const loaded = loadedBefore?.get(hostId);
							if (!loaded) {
								return deps;
							}
							const needed = deps.filter((dep) => !loaded.has(dep));
							return needed.length === 1 && needed[0] === file ? [] : needed;
						},
					},
				},
			};
		},
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
			loadedBefore = new Map(
				[...chunks.keys()].map((fileName) => [fileName, closure(fileName)]),
			);
			/** @type {Set<string>} */
			const entry = new Set();
			// The public plot and its embed are pages of their own, each with its own script.
			/** @type {Set<string>} */
			const perf = new Set();
			/** @type {Set<string>} */
			const embed = new Set();
			for (const chunk of chunks.values()) {
				const isEmbed =
					chunk.isEntry &&
					/\/src\/pages\/perf\/\[project\]\/embed\.astro\?/.test(
						chunk.facadeModuleId ?? "",
					);
				const isPerf =
					chunk.isEntry &&
					/\/src\/pages\/perf\/\[project\]\/index\.astro\?/.test(
						chunk.facadeModuleId ?? "",
					);
				// Astro's page script, which every page loads.
				const isPageScript = chunk.isEntry && chunk.name === "page";
				// The app's script, which every console page shares.
				const isApp =
					chunk.isEntry &&
					/\/src\/next\/astro\/Main\.astro\?/.test(chunk.facadeModuleId ?? "");
				if (isApp || isPageScript) {
					closure(chunk.fileName, entry);
				}
				if (isPerf || isPageScript) {
					closure(chunk.fileName, perf);
				}
				if (isEmbed || isPageScript) {
					closure(chunk.fileName, embed);
				}
			}
			/** @type {(fileName: string, loaded?: Set<string>) => string[]} */
			const own = (fileName, loaded = entry) =>
				[...closure(fileName)]
					.filter((file) => !loaded.has(file))
					.map((file) => `/${file}`);
			/** @type {Record<string, string[]>} */
			const pages = {};
			// What a page loads only for some of its links, such as Explore's plot.
			/** @type {Record<string, string[]>} */
			const parts = {};
			/** @type {Record<string, string[]>} */
			const perfParts = {};
			/** @type {Record<string, string[]>} */
			const embedParts = {};
			for (const chunk of chunks.values()) {
				const module = chunk.facadeModuleId?.match(
					/\/src\/next\/(\w+)\/(\w+)\.tsx?$/,
				);
				if (!(chunk.isDynamicEntry && module)) {
					continue;
				}
				const [, dir, name] = module;
				if (dir === "pages" && name) {
					pages[name] = own(chunk.fileName);
				} else {
					parts[`${dir}/${name}`] = own(chunk.fileName);
					perfParts[`${dir}/${name}`] = own(chunk.fileName, perf);
					embedParts[`${dir}/${name}`] = own(chunk.fileName, embed);
				}
			}
			preloads = {
				entry: [...entry].map((file) => `/${file}`),
				pages,
				parts,
				perf: {
					entry: [...perf].map((file) => `/${file}`),
					parts: perfParts,
				},
				embed: {
					entry: [...embed].map((file) => `/${file}`),
					parts: embedParts,
				},
			};
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
