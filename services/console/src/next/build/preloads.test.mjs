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
