import { mkdtemp, readFile, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { pathToFileURL } from "node:url";
import { describe, expect, test } from "vitest";
import nextPreloads from "./preloads.mjs";

/**
 * The preload list Vite would write for each lazy import once the client
 * build produced `chunks`, each a file name and the files it imports.
 *
 * @param {Record<string, string[]>} chunks
 */
const preloadsAfter = (chunks) => {
	/** @type {any} */
	let plugin;
	nextPreloads().hooks["astro:config:setup"]({
		updateConfig: (/** @type {any} */ config) => {
			[plugin] = config.vite.plugins;
		},
	});
	const bundle = Object.fromEntries(
		Object.entries(chunks).map(([fileName, imports]) => [
			fileName,
			{ type: "chunk", fileName, imports },
		]),
	);
	plugin.generateBundle.call({ environment: { name: "client" } }, {}, bundle);
	return plugin.configEnvironment("client").build.modulePreload
		.resolveDependencies;
};

describe("a lazy import's preload list", () => {
	const resolve = preloadsAfter({
		"assets/entry.js": ["assets/common.js"],
		"assets/common.js": ["assets/solid.js"],
		"assets/solid.js": [],
		"assets/Page.js": ["assets/common.js", "assets/own.js"],
		"assets/own.js": ["assets/solid.js"],
	});

	test("leaves out every file its importer loaded, through any depth of imports", () => {
		expect(
			resolve(
				"assets/Page.js",
				[
					"assets/Page.js",
					"assets/common.js",
					"assets/own.js",
					"assets/solid.js",
				],
				{ hostId: "assets/entry.js" },
			),
		).toEqual(["assets/Page.js", "assets/own.js"]);
	});

	test("is empty when the import's own file is all that is left to load", () => {
		expect(
			resolve("assets/Page.js", ["assets/Page.js", "assets/common.js"], {
				hostId: "assets/entry.js",
			}),
		).toEqual([]);
	});

	test("keeps every file for an importer the build did not report", () => {
		const deps = ["assets/Page.js", "assets/common.js"];
		expect(
			resolve("assets/Page.js", deps, { hostId: "assets/other.js" }),
		).toEqual(deps);
	});
});

describe("the pages' entries", () => {
	// Kills the public plot or its embed loading the console's entry, or none
	// of its own, and a classic page's script taken for the public plot's.
	test("the public plot and its embed each load their own script", async () => {
		const dir = await mkdtemp(join(tmpdir(), "preloads-"));
		const server = join(dir, "entry.mjs");
		await writeFile(server, 'const BUILT = "@@BENCHER_NEXT_PRELOADS@@";');
		/** @type {any} */
		let plugin;
		const integration = nextPreloads();
		integration.hooks["astro:config:setup"]({
			updateConfig: (/** @type {any} */ config) => {
				[plugin] = config.vite.plugins;
			},
		});
		integration.hooks["astro:config:done"]({
			config: { build: { server: pathToFileURL(`${dir}/`) } },
		});
		const page = (/** @type {string} */ file) =>
			`/repo/services/console/src/pages/${file}?astro&type=script&index=0&lang.ts`;
		/** @type {[string, string[], string | null][]} */
		const chunks = [
			[
				"assets/app.js",
				["assets/shared.js"],
				"/repo/services/console/src/next/astro/Main.astro?astro&type=script&index=0&lang.ts",
			],
			[
				"assets/perf.js",
				["assets/shared.js"],
				page("perf/[project]/index.astro"),
			],
			["assets/embed.js", [], page("perf/[project]/embed.astro")],
			["assets/plan.js", [], page("console/onboard/plan.astro")],
			["assets/shared.js", [], null],
		];
		plugin.generateBundle.call(
			{ environment: { name: "client" } },
			{},
			Object.fromEntries(
				chunks.map(([fileName, imports, facadeModuleId]) => [
					fileName,
					{ type: "chunk", fileName, imports, isEntry: true, facadeModuleId },
				]),
			),
		);
		await integration.hooks["astro:build:done"]();
		const built = JSON.parse(
			(await readFile(server, "utf8")).replace(/^const BUILT = |;$/g, ""),
		);
		expect(built.entry.sort()).toEqual(["/assets/app.js", "/assets/shared.js"]);
		expect(built.perf.entry.sort()).toEqual([
			"/assets/perf.js",
			"/assets/shared.js",
		]);
		expect(built.embed.entry).toEqual(["/assets/embed.js"]);
	});
});
