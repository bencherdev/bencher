import * as Sentry from "@sentry/astro";
import { dataCollection, scrubUrls } from "./src/util/sentry";

Sentry.init({
	dsn: import.meta.env.PUBLIC_SENTRY_DSN,
	environment: import.meta.env.SENTRY_ENVIRONMENT,
	release: import.meta.env.PUBLIC_VERCEL_GIT_COMMIT_SHA,
	tracesSampleRate: 1,
	dataCollection,
	beforeSend: scrubUrls,
	beforeSendTransaction: scrubUrls,
	beforeBreadcrumb: scrubUrls,
});
