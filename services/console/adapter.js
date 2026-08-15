import * as fs from "node:fs";

const adapter = process.argv[2];

if (!adapter || !["node", "cloudflare"].includes(adapter)) {
	console.error("Invalid adapter", adapter);
	process.exit(1);
}

const path = "./astro.config.mjs";
let file = fs.readFileSync(path, "utf8");
switch (adapter) {
	case "node":
		file = file.replace(
			'// import node from "@astrojs/node";',
			'import node from "@astrojs/node";',
		);
		file = file.replace(
			"adapter: undefined,",
			`adapter: node({ mode: "standalone" }),`,
		);
		break;
	case "cloudflare":
		file = file.replace(
			'// import cloudflare from "@astrojs/cloudflare";',
			'import cloudflare from "@astrojs/cloudflare";',
		);
		file = file.replace(
			"adapter: undefined,",
			`adapter: cloudflare({ imageService: "compile" }),`,
		);
		// The Sentry server SDK only supports Node runtimes,
		// so only the client SDK is enabled on Cloudflare Workers.
		// https://docs.sentry.io/platforms/javascript/guides/astro/
		file = file.replace(
			"enabled: undefined,",
			"enabled: { client: true, server: false },",
		);
		break;
}

fs.writeFileSync(path, file);
