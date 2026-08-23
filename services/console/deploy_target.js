import * as fs from "node:fs";
import * as path from "node:path";

// The Cloudflare adapter resolves `wrangler.jsonc` at build time and writes the
// deployable config here. `wrangler deploy` reads it through
// `.wrangler/deploy/config.json`, so `--env` on the deploy is ignored and the
// environment is whatever `CLOUDFLARE_ENV` selected during the build.
// A build that forgets `CLOUDFLARE_ENV` silently produces the production
// config, and nothing downstream would notice. Check before deploying.
const CONFIG = "./dist/server/wrangler.json";
// The redirect that sends `wrangler deploy` to CONFIG. It is a hidden file, so
// it is the part of the build most easily lost in transit: artifact uploads skip
// hidden files unless told otherwise. Losing it is silent rather than loud,
// because wrangler then falls back to the source `wrangler.jsonc` and deploys
// whatever `--env` says, which is exactly the mismatch this script prevents.
const DEPLOY_CONFIG = "./.wrangler/deploy/config.json";

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

if (!fs.existsSync(DEPLOY_CONFIG)) {
	console.error(
		`No ${DEPLOY_CONFIG}. Without it \`wrangler deploy\` ignores ${CONFIG} and falls back to \`wrangler.jsonc\`.`,
	);
	process.exit(1);
}

// Resolved the way wrangler resolves it: relative to the redirect's own directory.
const { configPath } = JSON.parse(fs.readFileSync(DEPLOY_CONFIG, "utf8"));
const redirected =
	configPath && path.resolve(path.dirname(DEPLOY_CONFIG), configPath);

if (redirected !== path.resolve(CONFIG)) {
	const target = redirected ? path.relative(".", redirected) : "nothing";
	console.error(
		`${DEPLOY_CONFIG} points at ${target}, not ${path.relative(".", CONFIG)}.`,
	);
	console.error(
		"The build output and the deploy redirect disagree. Rebuild the site: `npm run cloudflare`.",
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
