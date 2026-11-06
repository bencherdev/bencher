// @vitest-environment happy-dom
import { afterEach, expect, test } from "vitest";
import { type Turnstile, turnstileToken } from "./turnstile";

afterEach(() => {
	document.body.replaceChildren();
});

/** A Turnstile that answers each widget with `answer`, recording what it drew. */
const fake = (answer: "token" | "error" | "expired") => {
	const drawn: { sitekey: string; container: HTMLElement }[] = [];
	const removed: string[] = [];
	const turnstile: Turnstile = {
		render: (container, options) => {
			drawn.push({ sitekey: options.sitekey, container });
			const widget = `w${drawn.length}`;
			queueMicrotask(() => {
				if (answer === "token") {
					options.callback(`token-${widget}`);
				} else if (answer === "error") {
					options["error-callback"]();
				} else {
					options["expired-callback"]();
				}
			});
			return widget;
		},
		execute: () => {},
		remove: (widget) => removed.push(widget),
	};
	return { turnstile, drawn, removed };
};

// Kills a token reused across requests, a widget drawn for the wrong site, and
// one left on the page after it answered.
test("each token comes from a widget of its own, removed once it answers", async () => {
	const { turnstile, drawn, removed } = fake("token");
	const load = async () => turnstile;
	expect(await turnstileToken("site-key", load)).toBe("token-w1");
	expect(await turnstileToken("site-key", load)).toBe("token-w2");
	expect(drawn.map(({ sitekey }) => sitekey)).toEqual(["site-key", "site-key"]);
	expect(removed).toEqual(["w1", "w2"]);
	expect(drawn.every(({ container }) => !container.isConnected)).toBe(true);
});

// Kills a refused or expired check that hangs the door instead of failing.
test("a refused or expired check fails, and leaves nothing behind", async () => {
	for (const answer of ["error", "expired"] as const) {
		const { turnstile, drawn, removed } = fake(answer);
		await expect(
			turnstileToken("site-key", async () => turnstile),
		).rejects.toThrow();
		expect(removed).toEqual(["w1"]);
		expect(drawn[0]?.container.isConnected).toBe(false);
	}
	await expect(
		turnstileToken("site-key", async () => {
			throw new Error("Turnstile did not load");
		}),
	).rejects.toThrow("Turnstile did not load");
});
