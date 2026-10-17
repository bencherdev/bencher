import type { Event } from "@sentry/astro";
import { describe, expect, test } from "vitest";
import {
	type Attached,
	type Form,
	browserOf,
	feedbackArgs,
	scrub,
	sizeOf,
} from "./report";

const ABOUT = {
	url: "https://bencher.dev/next/console/projects/hashbrown/runs",
	slug: "hashbrown",
	email: "muriel@nowhere.example",
	build: "0.6.13",
};

const form = (off: Attached[] = [], extra: Partial<Form> = {}): Form => ({
	kind: "bug",
	message: "Runs shows Queued on the newest run.",
	off: new Set(off),
	...extra,
});

describe("feedbackArgs", () => {
	// Kills a report that drops the message, the kind, the page, the project,
	// the build, or the email while each is ticked.
	test("everything ticked goes into the call", () => {
		expect(feedbackArgs(form(), ABOUT)).toEqual({
			params: {
				message: "Runs shows Queued on the newest run.",
				url: "https://bencher.dev/next/console/projects/hashbrown/runs",
				email: "muriel@nowhere.example",
				tags: { kind: "bug", project: "hashbrown", build: "0.6.13" },
			},
			hint: {},
		});
	});

	// Kills an unticked page, project, build, or email still sent.
	test("each unticked item stays out of the call", () => {
		expect(
			feedbackArgs(
				form(["page", "project", "build", "email", "session"], {
					kind: "feedback",
				}),
				ABOUT,
			),
		).toEqual({
			params: {
				message: "Runs shows Queued on the newest run.",
				tags: { kind: "feedback" },
			},
			hint: {},
		});
	});

	// Kills an image left out of the call, or sent under another name or type.
	test("an image goes as an attachment", () => {
		const data = new Uint8Array([137, 80, 78, 71]);
		const { hint } = feedbackArgs(
			form([], {
				image: { filename: "runs-stuck.png", contentType: "image/png", data },
			}),
			ABOUT,
		);
		expect(hint).toEqual({
			attachments: [
				{ filename: "runs-stuck.png", contentType: "image/png", data },
			],
		});
	});
});

/** A feedback event as the browser SDK hands it to the transport. */
const event = (): Event => ({
	type: "feedback",
	release: "4f2a91c",
	environment: "production",
	transaction: "/next/console/projects/hashbrown/runs",
	request: {
		url: "https://bencher.dev/next/console/projects/hashbrown/runs",
		headers: {
			"User-Agent": "Mozilla/5.0",
			Referer: "https://bencher.dev/next/console/projects/hashbrown",
		},
	},
	contexts: {
		trace: { trace_id: "a".repeat(32), span_id: "b".repeat(16) },
		culture: { locale: "en-US", timezone: "UTC" },
		feedback: {
			message: "Runs shows Queued on the newest run.",
		},
	},
	tags: { kind: "bug" },
	breadcrumbs: [{ category: "navigation" }],
	sdkProcessingMetadata: {
		dynamicSamplingContext: {
			trace_id: "a".repeat(32),
			release: "4f2a91c",
			environment: "production",
			transaction: "/next/console/projects/hashbrown/runs",
		},
	},
});

describe("scrub", () => {
	// Kills a scrub that strips what is ticked.
	test("with everything ticked the event is untouched", () => {
		expect(scrub(event(), new Set())).toEqual(event());
	});

	// Kills an unticked page whose address still rides in the request, the
	// transaction, the sampling context, or the referrer.
	test("an unticked page leaves no address", () => {
		const scrubbed = scrub(event(), new Set(["page"]));
		expect(JSON.stringify(scrubbed)).not.toContain("/next/console");
		expect(scrubbed.request).toEqual({
			headers: { "User-Agent": "Mozilla/5.0" },
		});
	});

	// Kills an unticked browser whose user agent still rides in the request,
	// or whose locale and time zone still ride in the contexts.
	test("an unticked browser leaves no user agent or locale", () => {
		const scrubbed = scrub(event(), new Set(["browser"]));
		expect(scrubbed.contexts?.culture).toBeUndefined();
		expect(scrubbed.request?.headers).toEqual({
			Referer: "https://bencher.dev/next/console/projects/hashbrown",
		});
	});

	// Kills an unticked build whose release or environment still rides
	// on the event or its sampling context.
	test("an unticked build leaves no release", () => {
		const scrubbed = scrub(event(), new Set(["build"]));
		expect(JSON.stringify(scrubbed)).not.toMatch(/4f2a91c|production/);
	});

	// Kills an unticked session link whose trace or trail still ties the
	// report to the session.
	test("an unticked session link leaves no trace or trail", () => {
		const scrubbed = scrub(event(), new Set(["session"]));
		expect(JSON.stringify(scrubbed)).not.toMatch(/a{32}/);
		expect(scrubbed.breadcrumbs).toBeUndefined();
	});
});

describe("browserOf", () => {
	// Kills a parse that names Chrome for Edge or Safari for Chrome, or misses the system.
	test.each([
		[
			"Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36",
			"Chrome 131, Linux",
		],
		[
			"Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36 Edg/131.0.2903.86",
			"Edge 131, Windows",
		],
		[
			"Mozilla/5.0 (Macintosh; Intel Mac OS X 10.15; rv:131.0) Gecko/20100101 Firefox/131.0",
			"Firefox 131, macOS",
		],
		[
			"Mozilla/5.0 (iPhone; CPU iPhone OS 18_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.0 Mobile/15E148 Safari/604.1",
			"Safari 18, iOS",
		],
		[
			"Mozilla/5.0 (Linux; Android 14) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Mobile Safari/537.36",
			"Chrome 131, Android",
		],
	])("%s", (agent, shown) => {
		expect(browserOf(agent)).toBe(shown);
	});

	// Kills an unknown agent shown as nothing.
	test("an agent it cannot read shows as sent", () => {
		expect(browserOf("curl/8.0")).toBe("curl/8.0");
	});
});

// Kills a size shown in the wrong unit, or rounded to nothing.
test("sizeOf reads in KB, then MB", () => {
	expect(sizeOf(300)).toBe("1 KB");
	expect(sizeOf(184 * 1024)).toBe("184 KB");
	expect(sizeOf(3.5 * 1024 * 1024)).toBe("3.5 MB");
});
