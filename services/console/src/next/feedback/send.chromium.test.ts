import * as Sentry from "@sentry/astro";
import { beforeAll, expect, test } from "vitest";
import type { Attached } from "./report";
import { send } from "./send";

/** Every envelope the SDK sent, as text, while `failing` is off. */
const sent: string[] = [];
let failing = false;

beforeAll(async () => {
	// Test files share this tab, and the replay resumes a session another stored.
	sessionStorage.clear();
	Sentry.init({
		dsn: "http://feedback@127.0.0.1:9/1",
		release: "test-build",
		environment: "test",
		tracesSampleRate: 1,
		replaysSessionSampleRate: 1,
		integrations: [
			Sentry.browserTracingIntegration(),
			Sentry.replayIntegration(),
		],
		transport: (options: Parameters<typeof Sentry.makeFetchTransport>[0]) =>
			Sentry.createTransport(options, async ({ body }) => {
				if (failing) {
					throw new TypeError("Failed to fetch");
				}
				sent.push(
					typeof body === "string" ? body : new TextDecoder().decode(body),
				);
				return { statusCode: 200 };
			}),
	});
});

/** The envelope that carries the feedback with this message, header and event. */
const feedbackEnvelope = (message: string) => {
	const envelope = sent.find((body) => body.includes(message));
	const [header, , item] = envelope?.split("\n") ?? [];
	return {
		header: JSON.parse(header ?? "{}"),
		event: JSON.parse(item ?? "{}"),
		text: envelope ?? "",
	};
};

const ABOUT = {
	url: location.href,
	slug: "hashbrown",
	email: "muriel@nowhere.example",
	build: "0.6.13",
};
const ALL: Attached[] = [
	"page",
	"project",
	"browser",
	"build",
	"session",
	"email",
];

// Kills a scrub run before the SDK adds the trace, the replay, the address,
// or the release (as a scope's event processor is), which leaves them in what
// is sent, and a scrub that never runs.
test("an unticked item is absent from the envelope the SDK sends", async () => {
	await expect.poll(() => Sentry.getReplay()?.getReplayId()).toBeDefined();
	await send({ kind: "bug", message: "all ticked", off: new Set() }, ABOUT);
	await send({ kind: "bug", message: "none ticked", off: new Set(ALL) }, ABOUT);

	const ticked = feedbackEnvelope("all ticked");
	expect(ticked.event.contexts.trace.trace_id).toMatch(/^[0-9a-f]{32}$/);
	expect(ticked.event.contexts.feedback.replay_id).toMatch(/^[0-9a-f]{32}$/);
	expect(ticked.header.trace.trace_id).toMatch(/^[0-9a-f]{32}$/);
	for (const attached of [
		location.href,
		navigator.userAgent,
		"test-build",
		"hashbrown",
		"muriel@nowhere.example",
		"timezone",
	]) {
		expect(ticked.text).toContain(attached);
	}

	const unticked = feedbackEnvelope("none ticked");
	expect(unticked.event.contexts.trace).toBeUndefined();
	expect(unticked.event.contexts.feedback.replay_id).toBeUndefined();
	expect(unticked.header.trace).toBeUndefined();
	for (const attached of [
		location.href,
		navigator.userAgent,
		"test-build",
		"hashbrown",
		"muriel@nowhere.example",
		"timezone",
		Sentry.getReplay()?.getReplayId() ?? "replay",
	]) {
		expect(unticked.text).not.toContain(attached);
	}
});

// Kills a send that reports success when the browser blocked the request.
test("a report the transport could not deliver rejects", async () => {
	failing = true;
	await expect(
		send({ kind: "bug", message: "blocked", off: new Set() }, ABOUT),
	).rejects.toThrow();
	failing = false;
});
