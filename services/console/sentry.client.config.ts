import { deferErrors, whenIdle } from "./src/util/deferredErrors";

// The SDK arrives in its own chunk, so no page waits on it to paint. The new
// console's pages and the auth pages wait longer, until the page has loaded and
// the browser is idle.
// Errors raised before it arrives are kept and sent once it does.
const errors = deferErrors(window);

const load = () => {
	const options = import("./src/util/sentry");
	return import("@sentry/astro").then(async (Sentry) => {
		const { browserOptions } = await options;
		Sentry.init({
			dsn: import.meta.env.PUBLIC_SENTRY_DSN,
			environment: import.meta.env.SENTRY_ENVIRONMENT,
			release: import.meta.env.PUBLIC_VERCEL_GIT_COMMIT_SHA,
			// Each integration by name, so the bundle keeps only these of the SDK.
			...browserOptions(
				{
					tracing: Sentry.browserTracingIntegration,
					replay: Sentry.replayIntegration,
				},
				location.pathname,
				import.meta.env.PUBLIC_SENTRY_NO_REPLAY === "true",
			),
		});
		errors.flush((error) => Sentry.captureException(error));
	});
};

if (document.querySelector("#console, #public, #embed, #auth")) {
	whenIdle(load);
} else {
	load();
}
