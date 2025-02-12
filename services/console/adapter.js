import * as fs from "node:fs";

const adapter = process.argv[2];

if (!adapter || !["node", "netlify"].includes(adapter)) {
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
	case "netlify":
		file = file.replace(
			'// import netlify from "@astrojs/netlify";',
			'import netlify from "@astrojs/netlify";',
		);
		file = file.replace("adapter: undefined,", "adapter: netlify(),");
		break;
}

fs.writeFileSync(path, file);
