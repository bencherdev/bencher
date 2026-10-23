import * as fs from "node:fs";
import * as path from "node:path";

// `/assets/` is cached as immutable, so a WASM module the client loads by a fixed name outlives a deploy.
const CLIENT = "./dist/client";
const ASSETS = path.join(CLIENT, "assets");
const WASM_URL = /["'`]([^"'`\s]+\.wasm)["'`]/g;
const HASHED = /^(.+)\.[\w-]{8,}\.wasm$/;

if (!fs.existsSync(ASSETS)) {
	console.error(`No ${ASSETS}. Build the site first.`);
	process.exit(1);
}

// wasm-bindgen's glue keeps its fixed-name default URL even when it is handed a
// hashed one, so a module passes when the client also references a hashed copy.
const modules = new Map();
for (const file of fs.readdirSync(ASSETS, { recursive: true })) {
	if (!file.endsWith(".js")) {
		continue;
	}
	const chunk = path.join(ASSETS, file);
	for (const [, url] of fs.readFileSync(chunk, "utf8").matchAll(WASM_URL)) {
		const name = path.posix.basename(url);
		const hashed = name.match(HASHED);
		const module = hashed ? hashed[1] : name.slice(0, -".wasm".length);
		const resolved = url.startsWith("/")
			? path.join(CLIENT, url)
			: path.join(path.dirname(chunk), url);
		const urls = modules.get(module) ?? { hashed: [], fixed: [] };
		if (hashed && fs.existsSync(resolved)) {
			urls.hashed.push(url);
		} else {
			urls.fixed.push(url);
		}
		modules.set(module, urls);
	}
}

if (modules.size === 0) {
	console.error(`No client chunk under ${ASSETS} references a WASM module.`);
	process.exit(1);
}

let failed = false;
for (const [module, { hashed, fixed }] of modules) {
	if (hashed.length === 0) {
		console.error(
			`The client loads the ${module} WASM only from ${fixed.join(", ")}, none a content-hashed file the build wrote.`,
		);
		failed = true;
	} else {
		console.log(`The ${module} WASM loads from ${hashed.join(", ")}.`);
	}
}
if (failed) {
	process.exit(1);
}
