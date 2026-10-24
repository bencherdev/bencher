/**
 * Name a build's Sentry environment after the deploy `CLOUDFLARE_ENV` selects,
 * where an empty or unset value selects the top level, production.
 *
 * @param {string | undefined} cloudflareEnv
 */
export const sentryEnvironment = (cloudflareEnv) =>
	cloudflareEnv || "production";
