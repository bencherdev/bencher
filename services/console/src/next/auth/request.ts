import type { SignInRefusal } from "../../types/bencher";

/** A sign-in endpoint's `error_code`; any other value is no refusal this page knows. */
export type RefusalCode = `${SignInRefusal}`;

// Values, not the enum: importing it would put the shared types in every door's bundle.
const CODES: ReadonlySet<string> = new Set<RefusalCode>([
	"cross_origin",
	"too_many_attempts",
	"turnstile_failed",
	"invalid_invite",
	"invite_for_another_email",
	"unknown_organization",
	"email_failed",
	"sign_in_ended",
	"wrong_code",
	"no_tries_left",
	"codes_paused",
	"resend_waiting",
	"locked",
	"asked_elsewhere",
	"email_change_expired",
	"email_change_used",
]);

const TRIES_LEFT = "Bencher-Tries-Left";

export interface Refused {
	/** None when the request never reached the API. */
	status: number | undefined;
	/** None for a code this page does not know: a 500 carries an issue's ID there. */
	code: RefusalCode | undefined;
	/** Seconds until another try, from `Retry-After`. */
	retryAfter: number | undefined;
	triesLeft: number | undefined;
}

export type Answer<T> = { ok: true; data: T } | { ok: false; refused: Refused };

/** Ask the API, through the console's relay, as the browser's own cookies. */
export const ask = async <T>(
	method: "GET" | "POST" | "PUT" | "DELETE",
	path: string,
	body?: unknown,
	fetch = globalThis.fetch.bind(globalThis),
): Promise<Answer<T>> => {
	let response: Response;
	try {
		response = await fetch(`/api${path}`, {
			method,
			headers: body === undefined ? {} : { "Content-Type": "application/json" },
			...(body === undefined ? {} : { body: JSON.stringify(body) }),
		});
	} catch {
		return {
			ok: false,
			refused: {
				status: undefined,
				code: undefined,
				retryAfter: undefined,
				triesLeft: undefined,
			},
		};
	}
	// An unread `no-store` body holds its connection.
	const text = await response.text().catch(() => "");
	if (!response.ok) {
		return {
			ok: false,
			refused: {
				status: response.status,
				code: codeOf(text),
				retryAfter: count(response.headers.get("Retry-After")),
				triesLeft: count(response.headers.get(TRIES_LEFT)),
			},
		};
	}
	return { ok: true, data: (text ? JSON.parse(text) : undefined) as T };
};

const codeOf = (text: string): RefusalCode | undefined => {
	try {
		const code: unknown = JSON.parse(text)?.error_code;
		return typeof code === "string" && CODES.has(code)
			? (code as RefusalCode)
			: undefined;
	} catch {
		return undefined;
	}
};

const count = (value: string | null): number | undefined => {
	const parsed = value === null ? Number.NaN : Number(value);
	return Number.isInteger(parsed) && parsed >= 0 ? parsed : undefined;
};
