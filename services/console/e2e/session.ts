import { randomUUID } from "node:crypto";
import type { JsonUser, JsonUserKeyCreated } from "../src/types/bencher";
import { unique } from "./api";
import { RELAY, type Reader, SESSION_COOKIE, expect } from "./fixtures";
import { mailMark, mailedCode } from "./mail";

const PENDING_COOKIE = "__Host-bencher_pending";

interface Answer<T> {
	status: number;
	data: T;
	/** Each cookie the answer sets, by name. */
	cookies: Map<string, string>;
}

/**
 * Ask the API through the console's relay as a page of the console does, in
 * the session or the pending sign-in `cookie` names. Node's own `fetch` keeps
 * no cookies, so each request carries only the ones it names.
 */
export const fromConsole = async <T>(
	method: "GET" | "POST" | "PATCH" | "PUT" | "DELETE",
	path: string,
	{ cookie, body }: { cookie?: [string, string]; body?: unknown } = {},
): Promise<Answer<T>> => {
	const headers: Record<string, string> = { "Sec-Fetch-Site": "same-origin" };
	if (cookie) {
		headers.Cookie = `${cookie[0]}=${cookie[1]}`;
	}
	if (body !== undefined) {
		headers["Content-Type"] = "application/json";
	}
	const response = await fetch(`${RELAY}${path}`, {
		method,
		headers,
		...(body === undefined ? {} : { body: JSON.stringify(body) }),
	});
	const text = await response.text();
	const cookies = new Map(
		response.headers.getSetCookie().flatMap((set) => {
			const [pair = ""] = set.split(";");
			const at = pair.indexOf("=");
			return at > 0 ? [[pair.slice(0, at), pair.slice(at + 1)] as const] : [];
		}),
	);
	return {
		status: response.status,
		data: (text ? JSON.parse(text) : undefined) as T,
		cookies,
	};
};

/** `fromConsole` in `reader`'s session, for what only a browser's session may do. */
export const inSession = <T>(
	reader: Pick<Reader, "session">,
	method: "GET" | "POST" | "PATCH" | "PUT" | "DELETE",
	path: string,
	body?: unknown,
) =>
	fromConsole<T>(method, path, {
		cookie: [SESSION_COOKIE, reader.session],
		body,
	});

/** A new session for `email`, signed in as a browser signs in: by the email's code. */
export const signInByEmail = async (
	email: string,
): Promise<{ session: string; user: JsonUser; new: boolean }> => {
	const mark = mailMark();
	const started = await fromConsole("POST", "/v0/auth/email", {
		body: { email },
	});
	expect(started.status).toBe(202);
	const pending = started.cookies.get(PENDING_COOKIE);
	expect(pending).toBeTruthy();
	const code = await mailedCode(email, mark);
	const proved = await fromConsole<{ user: JsonUser; new: boolean }>(
		"POST",
		"/v0/auth/code",
		{ cookie: [PENDING_COOKIE, pending ?? ""], body: { code } },
	);
	expect(proved.status).toBe(200);
	const session = proved.cookies.get(SESSION_COOKIE);
	expect(session).toBeTruthy();
	return {
		session: session ?? "",
		user: proved.data.user,
		new: proved.data.new,
	};
};

/** A new user, signed up by email, named `name`, and holding a user key for the suite's requests. */
export const signUpAs = async (
	email: string,
	name: string,
): Promise<Reader> => {
	const { session, user } = await signInByEmail(email);
	const named = await inSession<JsonUser>(
		{ session },
		"PATCH",
		`/v0/users/${user.uuid}`,
		{ name },
	);
	expect(named.status, JSON.stringify(named.data)).toBe(200);
	const key = await inSession<JsonUserKeyCreated>(
		{ session },
		"POST",
		`/v0/users/${user.uuid}/keys`,
		{ name: "e2e" },
	);
	expect(key.status, JSON.stringify(key.data)).toBe(201);
	return { user: named.data, session, key: key.data.key };
};

/** A user of the test's own, signed up by email. */
export const signUp = ({
	name = unique("Le Quack"),
	// Not a bare UUID, which a path would read as the user's UUID, not their slug.
	email = `le-quack-${randomUUID()}@nowhere.com`,
}: {
	name?: string;
	email?: string;
} = {}) => signUpAs(email, name);
