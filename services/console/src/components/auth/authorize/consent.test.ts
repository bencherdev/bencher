import { describe, expect, test } from "vitest";
import {
	ConsentError,
	Credential,
	PastedKey,
	Refusal,
	answerUrl,
	consentRequest,
	pastedKey,
	refusal,
} from "./consent";

const UUID = "0b5c4f6e-2a1d-4c3b-9e8f-7a6b5c4d3e2f";

describe("consentRequest", () => {
	const read = (query: string) => consentRequest(new URLSearchParams(query));

	// Kills a page that answers a request it cannot name exactly.
	test("takes exactly one valid request", () => {
		expect(read(`request=${UUID}`)).toEqual({ uuid: UUID });
		expect(read(`request=${UUID.toUpperCase()}`)).toEqual({ uuid: UUID });
		for (const query of [
			"",
			"request=",
			"request=not-a-uuid",
			`request=${UUID}x`,
			`request=x${UUID}`,
			`request=${UUID}&request=${UUID}`,
		]) {
			expect(read(query)).toEqual({ error: ConsentError.Malformed });
		}
	});

	// Kills a page that shows whatever the link says, or that answers a request
	// alongside an error.
	test("maps an error to one of its own", () => {
		for (const error of [
			ConsentError.InvalidRequest,
			ConsentError.InvalidClient,
			ConsentError.TemporarilyUnavailable,
			ConsentError.InvalidRedirectUri,
		]) {
			expect(read(`error=${error}&request=${UUID}`)).toEqual({ error });
		}
		for (const query of [
			"error=access_denied",
			"error=Sign+in+at+evil.example",
			"error=malformed",
			`error=${ConsentError.InvalidClient}&error=${ConsentError.InvalidRequest}`,
		]) {
			expect(read(query)).toEqual({ error: ConsentError.Other });
		}
	});
});

describe("pastedKey", () => {
	// Kills a page that sends a project key, or anything but a user key.
	test("tells a user key from a project key", () => {
		expect(pastedKey(`bencher_user_${"a".repeat(30)}`)).toBe(PastedKey.User);
		expect(pastedKey(`bencher_run_${"a".repeat(30)}`)).toBe(PastedKey.Project);
		expect(pastedKey("eyJhbGciOiJIUzI1NiJ9.e30.sig")).toBe(PastedKey.Other);
	});
});

describe("refusal", () => {
	// Kills a page that shows one state for every refusal, or a key's refusal as a
	// session that ended.
	test("names each refusal by status and credential", () => {
		expect(refusal(404, undefined)).toBe(Refusal.Gone);
		expect(refusal(409, Credential.Session)).toBe(Refusal.Answered);
		expect(refusal(429, Credential.Key)).toBe(Refusal.Busy);
		expect(refusal(400, Credential.Session)).toBe(Refusal.SessionEnded);
		expect(refusal(401, Credential.Session)).toBe(Refusal.SessionEnded);
		expect(refusal(403, Credential.Session)).toBe(Refusal.AccountRefused);
		expect(refusal(401, Credential.Key)).toBe(Refusal.KeyRefused);
		expect(refusal(403, Credential.Key)).toBe(Refusal.KeyRefused);
		expect(refusal(400, undefined)).toBe(Refusal.Failed);
		expect(refusal(401, undefined)).toBe(Refusal.Failed);
		expect(refusal(500, Credential.Session)).toBe(Refusal.Failed);
		expect(refusal(undefined, undefined)).toBe(Refusal.Failed);
	});
});

describe("answerUrl", () => {
	// Kills a page that sends the browser to a script or a page of its own making.
	test("is the API's URL as given, only on the web", () => {
		const url = "https://client.example/callback?code=bencher_code_x&state=s";
		expect(answerUrl(url)).toBe(url);
		expect(answerUrl("http://127.0.0.1:61016/callback?code=c")).toBe(
			"http://127.0.0.1:61016/callback?code=c",
		);
		for (const value of [
			"javascript:alert(1)",
			"data:text/html,hi",
			"/console",
			"",
			undefined,
			{},
		]) {
			expect(answerUrl(value)).toBeUndefined();
		}
	});
});
