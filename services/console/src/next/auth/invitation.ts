import type { JsonInvitation } from "../../types/bencher";
import { type Answer, ask } from "./request";

/** An invitation's secret: 32 bytes, unpadded base64url. */
const SECRET = /^[\w-]{43}$/;

/**
 * What the invitation `secret` names offers, read through the API, which
 * tells a signed in reader whether it is for them; a secret of the wrong
 * shape is refused here, as the API refuses one no invitation has.
 */
export const readInvitation = (
	secret: string,
	fetch?: typeof globalThis.fetch,
): Promise<Answer<JsonInvitation>> =>
	SECRET.test(secret)
		? ask<JsonInvitation>("POST", "/v0/auth/invite", { secret }, fetch)
		: Promise.resolve({
				ok: false,
				refused: {
					status: 400,
					code: "invalid_invite",
					retryAfter: undefined,
					triesLeft: undefined,
				},
			});

/** A role as the reader reads it. */
export const roleName = (role: string) =>
	role.charAt(0).toUpperCase() + role.slice(1);
