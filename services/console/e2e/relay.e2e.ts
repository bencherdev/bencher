import { randomUUID } from "node:crypto";
import { readFileSync } from "node:fs";
import type { Page } from "@playwright/test";
import { expect, seed, signedOut, test } from "./fixtures";
import { mailedCode } from "./mail";

test.use({ storageState: signedOut });

interface Answer {
	status: number;
	headers: Record<string, string>;
	body: string;
}

/** What the page's own `fetch` of `path` answered, a JSON `body` sent with `method`. */
const fetchFrom = (
	page: Page,
	path: string,
	send?: { method: string; body: unknown },
): Promise<Answer> =>
	page.evaluate(
		async ([path, send]) => {
			const response = await fetch(
				path,
				send && {
					method: send.method,
					headers: { "Content-Type": "application/json" },
					body: JSON.stringify(send.body),
				},
			);
			return {
				status: response.status,
				headers: Object.fromEntries(response.headers),
				body: await response.text(),
			};
		},
		[path, send] as const,
	);

/** The address the API's per-IP limits read for request `id`, once the API has answered it. */
const remoteIp = async (id: string | undefined) => {
	let lines: Record<string, unknown>[] = [];
	await expect
		.poll(() => {
			lines = readFileSync(seed.api_log, "utf8")
				.split("\n")
				.flatMap((line) => {
					try {
						const json = JSON.parse(line);
						return json?.req_id === id ? [json] : [];
					} catch {
						return [];
					}
				});
			return lines.some((line) => line.msg === "request completed");
		})
		.toBe(true);
	return lines.find((line) => line.msg === "Remote IP")?.remote_ip;
};

// Kills a relay that loses the session's `Set-Cookie`, the browser's `Cookie`,
// or the headers its same-origin check reads, one that drops the sign in's
// `Retry-After` or `Bencher-Tries-Left`, a route that ignores
// `INTERNAL_API_URL`, and an API that honors a cookie from a relay without its
// secret.
test("a session the relay sets signs in through it, and through no relay without the API's secret", async ({
	page,
	context,
}) => {
	const email = `${randomUUID()}@nowhere.com`;
	await page.goto("/api/v0/server/version");
	const started = await fetchFrom(page, "/api/v0/auth/email", {
		method: "POST",
		body: { email },
	});
	expect(started.status).toBe(202);
	const resend = await fetchFrom(page, "/api/v0/auth/email/resend", {
		method: "POST",
		body: {},
	});
	expect(resend.status).toBe(429);
	expect(Number(resend.headers["retry-after"])).toBeGreaterThan(0);

	const code = await mailedCode(email);
	const wrong = await fetchFrom(page, "/api/v0/auth/code", {
		method: "POST",
		body: { code: code === "000000" ? "000001" : "000000" },
	});
	expect(wrong.status).toBe(403);
	expect(wrong.headers["bencher-tries-left"]).toBe("4");
	const right = await fetchFrom(page, "/api/v0/auth/code", {
		method: "POST",
		body: { code },
	});
	expect(right.status).toBe(200);
	const cookies = await context.cookies(seed.console_url);
	expect(
		cookies.find((cookie) => cookie.name === "__Host-bencher_session"),
	).toMatchObject({ httpOnly: true, secure: true, sameSite: "Strict" });

	const session = await fetchFrom(page, "/api/v0/auth/session");
	expect(session.status).toBe(200);
	expect(JSON.parse(session.body).user.email).toBe(email);

	// The second console shares the host, so the browser sends it the same cookie.
	await page.goto(`${seed.wrong_secret_console_url}/api/v0/server/version`);
	const sent = page.waitForRequest((request) =>
		request.url().endsWith("/api/v0/auth/session"),
	);
	const refused = await fetchFrom(page, "/api/v0/auth/session");
	expect((await (await sent).allHeaders()).cookie).toContain(
		"__Host-bencher_session=",
	);
	expect(refused.status).toBe(401);
});

// Kills a relay that passes on an address or a secret the reader sent, a route
// that ignores `INTERNAL_API_URL`, and an API that reads the address a relay
// without its secret sends.
test("per-IP limits see the reader's address, and only through a relay with the API's secret", async ({
	request,
}) => {
	const forged = {
		"Bencher-Client-IP": "203.0.113.7",
		"Bencher-Relay-Secret": "forged",
		"X-Forwarded-For": "203.0.113.8",
		"Fly-Client-IP": "203.0.113.9",
		"CF-Connecting-IP": "203.0.113.10",
	};
	const path = `/api/v0/projects/${seed.projects.hashbrown.slug}`;

	const relayed = await request.get(`${seed.console_url}${path}`, {
		headers: forged,
	});
	expect(relayed.status()).toBe(200);
	expect(await remoteIp(relayed.headers()["x-request-id"])).toBe("127.0.0.1");

	const unshared = await request.get(
		`${seed.wrong_secret_console_url}${path}`,
		{ headers: forged },
	);
	expect(unshared.status()).toBe(200);
	expect(await remoteIp(unshared.headers()["x-request-id"])).toBeUndefined();
});
