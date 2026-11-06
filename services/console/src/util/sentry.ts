import type {
	BrowserOptions,
	browserTracingIntegration,
	replayIntegration,
} from "@sentry/astro";
import { FILTERED, reportSecrets, scrub, scrubber } from "./scrub";

const PII = ["forwarded", "-ip", "remote-", "via", "-user"];

// What Sentry 10 collected with `sendDefaultPii` unset, since Sentry 11 collects more by default.
export const dataCollection: NonNullable<BrowserOptions["dataCollection"]> = {
	userInfo: false,
	// The API's own cookies, which the relay puts on the console's origin.
	cookies: { deny: [...PII, "bencher_"] },
	httpHeaders: { request: { deny: PII }, response: { deny: PII } },
	httpBodies: [],
	urlQueryParams: {
		deny: [...PII, "code", "state", "token", "invite", "request"],
	},
	graphQL: { document: true, variables: true },
	genAI: { inputs: false, outputs: false },
	databaseQueryData: false,
	queues: false,
	stackFrameVariables: true,
	frameContextLines: 7,
};

// The values a sign-in's, a provider's, or a consent request's URL carries,
// in a URL or a bare query, and a sign-in link's fragment. Here, so only the
// SDK's chunk loads them.
const QUERY_SECRET =
	/((?:^|[?&])(?:code|state|token|invite|request)=)[^&#\s"'<>]+/gi;
const FRAGMENT = /(\/[^\s"'<>#]*)#[^\s"'<>]+/g;

/** Filter codes, states, tokens, invites, consent requests, and fragments out of every URL in a value, at any depth. */
export const scrubUrls = <T>(value: T): T =>
	scrub(
		value,
		(text) =>
			text
				.replace(QUERY_SECRET, `$1${FILTERED}`)
				.replace(FRAGMENT, `$1#${FILTERED}`),
		new WeakSet<object>(),
	) as T;

/**
 * Filter a browser's report: the secrets of its URLs, every Bencher API key,
 * which a reader may paste, and whatever the page asked to keep out.
 */
const scrubReport = <T>(value: T): T =>
	scrubber(reportSecrets())(scrubUrls(value));

/** Whether a page records Sentry Replay: never an auth page, which shows codes and addresses. */
export const replays = (pathname: string, noReplay: boolean) =>
	!noReplay && pathname !== "/auth" && !pathname.startsWith("/auth/");

/**
 * Each page's options but where it reports, here so they load with the SDK
 * and not before a page paints: no Replay on an auth page, and no secret of a
 * URL in any event, transaction, or breadcrumb.
 */
export const browserOptions = (
	integrations: {
		tracing: typeof browserTracingIntegration;
		replay: typeof replayIntegration;
	},
	pathname: string,
	noReplay: boolean,
): BrowserOptions => {
	const replay = replays(pathname, noReplay);
	return {
		tracesSampleRate: 1,
		integrations: [
			integrations.tracing(),
			...(replay ? [integrations.replay()] : []),
		],
		replaysSessionSampleRate: replay ? 0.1 : 0,
		replaysOnErrorSampleRate: replay ? 1 : 0,
		dataCollection,
		beforeSend: scrubReport,
		beforeSendTransaction: scrubReport,
		beforeBreadcrumb: scrubReport,
	};
};
