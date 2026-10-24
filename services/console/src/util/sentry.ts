import type { BrowserOptions } from "@sentry/astro";

const PII = ["forwarded", "-ip", "remote-", "via", "-user"];

// What Sentry 10 collected with `sendDefaultPii` unset, since Sentry 11 collects more by default.
export const dataCollection: NonNullable<BrowserOptions["dataCollection"]> = {
	userInfo: false,
	cookies: { deny: PII },
	httpHeaders: { request: { deny: PII }, response: { deny: PII } },
	httpBodies: [],
	urlQueryParams: { deny: PII },
	graphQL: { document: true, variables: true },
	genAI: { inputs: false, outputs: false },
	databaseQueryData: false,
	queues: false,
	stackFrameVariables: true,
	frameContextLines: 7,
};
