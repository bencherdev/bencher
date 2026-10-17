import * as Sentry from "@sentry/astro";
import { expect, test } from "vitest";
import type { Attached } from "./report";
import { send } from "./send";

/** Every envelope's item types, as sent. */
const sent: string[] = [];

const ALL: Attached[] = [
	"page",
	"project",
	"browser",
	"build",
	"session",
	"email",
];

/** Changes the page every 200 ms for `ms`, so a replay has something to hold. */
const churn = async (ms: number) => {
	const end = Date.now() + ms;
	while (Date.now() < end) {
		document.body.append(document.createElement("div"));
		await new Promise((resolve) => setTimeout(resolve, 200));
	}
};

// Kills a report with the session link unticked that still uploads the
// replay buffered for errors, which the SDK flushes for any feedback.
test("an unticked session link uploads no buffered replay", async () => {
	// Test files share this tab, and the replay resumes a session another stored.
	sessionStorage.clear();
	Sentry.init({
		dsn: "http://feedback@127.0.0.1:9/1",
		replaysSessionSampleRate: 0,
		replaysOnErrorSampleRate: 1,
		integrations: [Sentry.replayIntegration({ useCompression: false })],
		transport: (options: Parameters<typeof Sentry.makeFetchTransport>[0]) =>
			Sentry.createTransport(options, async ({ body }) => {
				sent.push(
					typeof body === "string" ? body : new TextDecoder().decode(body),
				);
				return { statusCode: 200 };
			}),
	});
	await expect
		.poll(() => Sentry.getReplay()?.getRecordingMode())
		.toBe("buffer");
	await churn(2_000);

	await send(
		{ kind: "bug", message: "buffered", off: new Set(ALL) },
		{
			url: location.href,
			slug: "hashbrown",
			email: "e@nowhere.example",
			build: "0",
		},
	);
	await churn(6_000);

	expect(sent.some((body) => body.includes("buffered"))).toBe(true);
	expect(sent.filter((body) => body.includes('"type":"replay_event"'))).toEqual(
		[],
	);
	expect(Sentry.getReplay()?.getRecordingMode()).toBe("buffer");
}, 20_000);
