import * as fs from "node:fs";

// The Cloudflare adapter resolves `wrangler.jsonc` at build time and writes the
// deployable config here. `wrangler deploy` reads it through
// `.wrangler/deploy/config.json`, so `--env` on the deploy is ignored and the
// environment is whatever `CLOUDFLARE_ENV` selected during the build.
// A build that forgets `CLOUDFLARE_ENV` silently produces the production
// config, and nothing downstream would notice. Check before deploying.
const CONFIG = "./dist/server/wrangler.json";

const expected = process.argv[2];

if (!expected) {
	console.error("Usage: node deploy_target.js <worker-name>");
	process.exit(1);
}

if (!fs.existsSync(CONFIG)) {
	console.error(
		`No ${CONFIG}. Build the site before deploying: \`npm run cloudflare\`.`,
	);
	process.exit(1);
}

const { name, targetEnvironment } = JSON.parse(fs.readFileSync(CONFIG, "utf8"));

if (name !== expected) {
	console.error(
		`Refusing to deploy: built the "${name}" Worker but expected "${expected}".`,
	);
	console.error(
		`The build resolved CLOUDFLARE_ENV to ${targetEnvironment ? `"${targetEnvironment}"` : "the top level"}. Rebuild with the right CLOUDFLARE_ENV.`,
	);
	process.exit(1);
}

console.log(`Deploy target is the "${name}" Worker.`);
