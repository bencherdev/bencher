import { deferErrors, whenIdle } from "./src/util/deferredErrors";

// The SDK arrives in its own chunk, so no page waits on it to paint. The new
// console's pages wait longer, until the page has loaded and the browser is idle.
// Errors raised before it arrives are kept and sent once it does.
const errors = deferErrors(window);
const replay = import.meta.env.PUBLIC_SENTRY_NO_REPLAY !== "true";

const load = () =>
	import("@sentry/astro").then((Sentry) => {
		Sentry.init({
			dsn: import.meta.env.PUBLIC_SENTRY_DSN,
			environment: import.meta.env.PUBLIC_VERCEL_ENV,
			release: import.meta.env.PUBLIC_VERCEL_GIT_COMMIT_SHA,
			tracesSampleRate: 1,
			integrations: [
				Sentry.browserTracingIntegration(),
				Sentry.replayIntegration(),
			],
			replaysSessionSampleRate: replay ? 0.1 : 0,
			replaysOnErrorSampleRate: replay ? 1 : 0,
		});
		errors.flush((error) => Sentry.captureException(error));
	});

if (document.querySelector("#console, #public, #embed")) {
	whenIdle(load);
} else {
	load();
}
