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
		file = `import node from "@astrojs/node";\n${file}`;
		file = file.replace(
			"adapter: undefined,",
			`adapter: node({ mode: "standalone" }),`,
		);
		break;
	case "netlify":
		file = `import netlify from "@astrojs/netlify";\n${file}`;
		file = file.replace("adapter: undefined,", "adapter: netlify(),");
		break;
}

fs.writeFileSync(path, file);
