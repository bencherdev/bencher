import { expect, test } from "vitest";
import { ask } from "./request";

const answering =
	(answer: () => Response, sent: { url: string; init?: RequestInit }[] = []) =>
	async (url: string | URL | Request, init?: RequestInit) => {
		sent.push({ url: String(url), init });
		return answer();
	};

const refusal = (
	status: number,
	body: unknown,
	headers: Record<string, string> = {},
) => new Response(JSON.stringify(body), { status, headers });

// Kills a sign-in request sent anywhere but the console's own relay, with a
// body that is not its JSON, or a credential of its own.
test("asks through the relay with a JSON body and no Authorization", async () => {
	const sent: { url: string; init?: RequestInit }[] = [];
	const answer = await ask<{ email: string }>(
		"POST",
		"/v0/auth/email",
		{ email: "ada@example.com" },
		answering(() => new Response('{"email":"ada@example.com"}'), sent),
	);
	expect(answer).toEqual({ ok: true, data: { email: "ada@example.com" } });
	expect(sent).toHaveLength(1);
	expect(sent[0]?.url).toBe("/api/v0/auth/email");
	expect(sent[0]?.init?.method).toBe("POST");
	expect(sent[0]?.init?.body).toBe('{"email":"ada@example.com"}');
	const headers = new Headers(sent[0]?.init?.headers);
	expect(headers.get("content-type")).toBe("application/json");
	expect(headers.has("authorization")).toBe(false);
});

// Kills a refusal read without its code, the tries left, or the wait, and an
// answer an empty body breaks.
test("a refusal carries its code, the tries left, and the wait", async () => {
	expect(
		await ask(
			"POST",
			"/v0/auth/code",
			{ code: "000000" },
			answering(() =>
				refusal(
					403,
					{ error_code: "wrong_code", message: "Wrong code" },
					{ "Bencher-Tries-Left": "4" },
				),
			),
		),
	).toEqual({
		ok: false,
		refused: {
			status: 403,
			code: "wrong_code",
			retryAfter: undefined,
			triesLeft: 4,
		},
	});
	for (const code of ["resend_waiting", "too_many_attempts", "codes_paused"]) {
		expect(
			await ask(
				"POST",
				"/v0/auth/email/resend",
				{},
				answering(() =>
					refusal(429, { error_code: code }, { "Retry-After": "42" }),
				),
			),
		).toMatchObject({ ok: false, refused: { code, retryAfter: 42 } });
	}
	expect(
		await ask(
			"DELETE",
			"/v0/auth/email",
			undefined,
			answering(() => new Response(null, { status: 204 })),
		),
	).toEqual({ ok: true, data: undefined });
});

// A 500 carries an issue's ID where a refusal's code goes. Kills a page that
// takes any `error_code` for a refusal it knows, or a body it cannot read.
test("an error code this page does not know is none", async () => {
	for (const answer of [
		refusal(500, { error_code: "a1b2c3d4e5f6", message: "Issue" }),
		refusal(403, { error_code: "toString" }),
		refusal(400, { error_code: 7 }),
		new Response("<html>", { status: 502 }),
	]) {
		const asked = await ask("POST", "/v0/auth/code", {}, async () => answer);
		expect(asked).toMatchObject({ ok: false, refused: { code: undefined } });
	}
});

// The relay marks every answer `no-store`, so an unread body holds its
// connection. Kills a refusal whose body is left unread.
test("a refused answer's body is read", async () => {
	const answer = refusal(401, { error_code: "sign_in_ended" });
	await ask("POST", "/v0/auth/link", {}, async () => answer);
	expect(answer.bodyUsed).toBe(true);
});

// Kills a request that never reached the API taken for a refusal with a status.
test("a request that never reached the API has no status", async () => {
	expect(
		await ask("POST", "/v0/auth/email", {}, async () => {
			throw new TypeError("Failed to fetch");
		}),
	).toEqual({
		ok: false,
		refused: {
			status: undefined,
			code: undefined,
			retryAfter: undefined,
			triesLeft: undefined,
		},
	});
});
