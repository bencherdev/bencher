import { httpHeadersToSpanAttributes } from "@sentry/astro";
// The filter the server's SDK runs on a request's URL, by its data collection rules.
import { filterCollectedUrl } from "@sentry/core";
import { expect, test } from "vitest";
import { FILTERED, keepOutOfReports } from "./scrub";
import { browserOptions, dataCollection, replays, scrubUrls } from "./sentry";

// Kills a console server that hands Sentry the pending sign in cookie a console on plain HTTP
// sets, whose name no snippet of Sentry's own matches.
test("Sentry gets no Bencher cookie's value", () => {
	const attributes = httpHeadersToSpanAttributes(
		{ cookie: "bencher_pending=PENDING; bencher_session=SESSION; theme=dark" },
		dataCollection as Parameters<typeof httpHeadersToSpanAttributes>[1],
	);
	expect(attributes["http.request.header.cookie"]).toEqual([
		"bencher_pending=[Filtered]",
		"bencher_session=[Filtered]",
		"theme=dark",
	]);
});

// Kills a Sentry Replay of an auth page, which would record the address, the
// code boxes, and a pasted key, and one turned off anywhere else.
test("no auth page records a Replay", () => {
	for (const path of [
		"/auth",
		"/auth/login",
		"/auth/confirm",
		"/auth/authorize",
	]) {
		expect(replays(path, false), path).toBe(false);
	}
	for (const path of [
		"/",
		"/console/projects/hashbrown",
		"/authors",
		"/perf/x",
	]) {
		expect(replays(path, false), path).toBe(true);
	}
	expect(replays("/console", true)).toBe(false);
});

// Kills options that record a Replay of an auth page or turn it off
// elsewhere, and an event, a transaction, or a breadcrumb sent with a URL's
// code, state, or fragment.
test("each page's options record no auth page and scrub every URL", () => {
	const Sentry = {
		tracing: () => ({ name: "BrowserTracing" }),
		replay: () => ({ name: "Replay" }),
	} as unknown as Parameters<typeof browserOptions>[0];
	const names = (path: string) =>
		(
			browserOptions(Sentry, path, false).integrations as { name: string }[]
		).map(({ name }) => name);
	expect(names("/auth/confirm")).toEqual(["BrowserTracing"]);
	expect(names("/console")).toEqual(["BrowserTracing", "Replay"]);
	const auth = browserOptions(Sentry, "/auth/login", false);
	expect([
		auth.replaysSessionSampleRate,
		auth.replaysOnErrorSampleRate,
	]).toEqual([0, 0]);

	const options = browserOptions(Sentry, "/console", false);
	const url = "https://bencher.dev/auth/github?code=c0de&state=s7a7e";
	for (const scrub of [
		options.beforeSend,
		options.beforeSendTransaction,
		options.beforeBreadcrumb,
	] as ((value: unknown, hint: unknown) => unknown)[]) {
		expect(JSON.stringify(scrub({ request: { url } }, {}))).not.toMatch(
			/c0de|s7a7e/,
		);
	}
});

// Kills error reporting that sends a sign-in link's secret, a provider's code or
// state, an old link's token, or an invite from any URL in an event, and one
// that filters what carries no secret.
test("filters codes, states, tokens, invites, and fragments out of every URL", () => {
	const event = {
		request: { url: "https://bencher.dev/auth/confirm#s3cr3t" },
		transaction: "/auth/github?code=c0de&state=s7a7e",
		breadcrumbs: [
			{
				category: "navigation",
				data: { from: "/auth/confirm#s3cr3t", to: "/auth/confirm" },
			},
			{
				category: "fetch",
				data: {
					url: "/auth/confirm?TOKEN=t0ken&plan=pro",
					note: "invite at /auth/signup?invite=a.b.c#top",
				},
			},
		],
		tags: { page: "/console/projects/hashbrown?status=all" },
	};

	const scrubbed = scrubUrls(event);
	const text = JSON.stringify(scrubbed);
	for (const secret of ["s3cr3t", "c0de", "s7a7e", "t0ken", "a.b.c"]) {
		expect(text).not.toContain(secret);
	}
	expect(scrubbed.request.url).toBe(
		`https://bencher.dev/auth/confirm#${FILTERED}`,
	);
	expect(scrubbed.transaction).toBe(
		`/auth/github?code=${FILTERED}&state=${FILTERED}`,
	);
	expect(scrubbed.breadcrumbs[1]?.data.url).toBe(
		`/auth/confirm?TOKEN=${FILTERED}&plan=pro`,
	);
	expect(scrubbed.breadcrumbs[0]?.data.to).toBe("/auth/confirm");
	expect(scrubbed.tags.page).toBe("/console/projects/hashbrown?status=all");
});

// Kills a browser report that carries a pasted API key, or anything the page
// asked to keep out, such as a consent request's UUID in any case, and a
// filter that reads the page's secrets only once.
test("each report leaves out API keys and what the page keeps out", () => {
	const Sentry = {
		tracing: () => ({ name: "BrowserTracing" }),
		replay: () => ({ name: "Replay" }),
	} as unknown as Parameters<typeof browserOptions>[0];
	const { beforeSend } = browserOptions(Sentry, "/auth/authorize", false);
	const send = beforeSend as (event: unknown, hint: object) => unknown;
	const key = `bencher_user_${"Ab3".repeat(10)}`;
	const uuid = "0b5c4f6e-2a1d-4c3b-9e8f-7a6b5c4d3e2f";
	const event = () => ({
		message: `Bearer ${key} for ${uuid.toUpperCase()}`,
		transaction: `GET /v0/oauth/authorizations/${uuid}`,
	});
	try {
		keepOutOfReports([uuid], globalThis);
		const text = JSON.stringify(send(event(), {}));
		expect(text).not.toContain(key);
		expect(text.toLowerCase()).not.toContain(uuid);
		expect(text).toContain(FILTERED);
	} finally {
		keepOutOfReports([], globalThis);
	}
	expect(JSON.stringify(send(event(), {}))).toContain(uuid);
});

const CONSENT_REQUEST = "0b5c4f6e-2a1d-4c3b-9e8f-7a6b5c4d3e2f";
const CONSENT_URL = `https://bencher.dev/auth/authorize?request=${CONSENT_REQUEST}`;
/** A report of the consent page, whose request UUID is all it takes to answer it. */
const consentReport = () => ({
	transaction: `GET ${CONSENT_URL}`,
	request: { url: CONSENT_URL, query_string: `request=${CONSENT_REQUEST}` },
	breadcrumbs: [{ category: "navigation", data: { to: CONSENT_URL } }],
});

// Kills a browser report of the consent page that keeps the request's UUID
// in its URL, its query, or a breadcrumb, though the page kept nothing out.
test("a browser report leaves out the consent request", () => {
	const Sentry = {
		tracing: () => ({ name: "BrowserTracing" }),
		replay: () => ({ name: "Replay" }),
	} as unknown as Parameters<typeof browserOptions>[0];
	const options = browserOptions(Sentry, "/auth/authorize", false);
	for (const filter of [
		options.beforeSend,
		options.beforeSendTransaction,
		options.beforeBreadcrumb,
	] as ((value: unknown, hint: unknown) => unknown)[]) {
		expect(JSON.stringify(filter(consentReport(), {}))).not.toContain(
			CONSENT_REQUEST,
		);
	}
});

// Kills a server report of the server-drawn consent page that keeps the
// request's UUID: in what the server's SDK collects of the URL, or in the
// transaction its filters send.
test("a server report leaves out the consent request", () => {
	const client = {
		getDataCollectionOptions: () => dataCollection,
	} as unknown as Parameters<typeof filterCollectedUrl>[1];
	const collected = filterCollectedUrl(CONSENT_URL, client) ?? "";
	expect(collected).toMatch(
		/^https:\/\/bencher\.dev\/auth\/authorize\?request=/,
	);
	expect(collected).not.toContain(CONSENT_REQUEST);
	expect(JSON.stringify(scrubUrls(consentReport()))).not.toContain(
		CONSENT_REQUEST,
	);
});
