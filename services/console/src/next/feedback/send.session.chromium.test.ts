import * as Sentry from "@sentry/astro";
import { expect, test } from "vitest";
import type { Attached } from "./report";
import { send } from "./send";

const sent: string[] = [];

const ALL: Attached[] = [
	"page",
	"project",
	"browser",
	"build",
	"session",
	"email",
];

/** Changes the page every 200 ms for `ms`, so a replay has something to record. */
const churn = async (ms: number) => {
	const end = Date.now() + ms;
	while (Date.now() < end) {
		document.body.append(document.createElement("div"));
		await new Promise((resolve) => setTimeout(resolve, 200));
	}
};

// Kills a report with the session link unticked that the recording replay
// still names, as the SDK's breadcrumb for any feedback does.
test("an unticked session link leaves the report out of the recording", async () => {
	// Test files share this tab, and the replay resumes a session another stored.
	sessionStorage.clear();
	Sentry.init({
		dsn: "http://feedback@127.0.0.1:9/1",
		replaysSessionSampleRate: 1,
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
		.toBe("session");
	await churn(2_000);

	await send(
		{ kind: "bug", message: "recorded", off: new Set(ALL) },
		{
			url: location.href,
			slug: "hashbrown",
			email: "e@nowhere.example",
			build: "0",
		},
	);
	await churn(7_000);

	const report = sent.find((body) => body.includes("recorded"));
	const id = /"event_id":"([0-9a-f]{32})"/.exec(report ?? "")?.[1];
	expect(id).toBeDefined();
	expect(
		sent.filter((body) => body !== report && body.includes(id ?? "")),
	).toEqual([]);
	expect(sent.some((body) => body.includes('"type":"replay_event"'))).toBe(
		true,
	);
	expect(Sentry.getReplay()?.getRecordingMode()).toBe("session");
}, 20_000);
