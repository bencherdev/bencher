import { requestUuid } from "../../components/auth/authorize/path";
import type { Carry } from "./pending";

const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;
const JWT = /^[\w-]+\.[\w-]+\.[\w-]+$/;
// Values, not the enum: importing it would put the shared types in every door's bundle.
const PLANS: ReadonlySet<string> = new Set([
	"free",
	"pro",
	"team",
	"enterprise",
]);

/** What the door's address asks a sign-in to carry, each part only in its own shape. */
export const carryOf = (query: URLSearchParams): Carry => {
	const carry: Carry = {};
	const invite = query.get("invite");
	if (invite && JWT.test(invite)) {
		carry.invite = invite;
	}
	const claim = query.get("claim");
	if (claim && UUID.test(claim)) {
		carry.claim = claim.toLowerCase();
	}
	const plan = query.get("plan");
	if (plan && PLANS.has(plan)) {
		carry.plan = plan;
	}
	const authorization = requestUuid(query.get("authorization"));
	if (authorization) {
		carry.authorization = authorization;
	}
	return carry;
};

/** The address a token names, as an invite's or an old link's names it. */
export const tokenEmail = (token: string | undefined): string | undefined => {
	const payload = token?.split(".")[1];
	if (!payload) {
		return undefined;
	}
	try {
		const base64 = payload.replace(/-/g, "+").replace(/_/g, "/");
		const bytes = Uint8Array.from(atob(base64), (char) => char.charCodeAt(0));
		const sub: unknown = JSON.parse(new TextDecoder().decode(bytes))?.sub;
		return typeof sub === "string" && sub ? sub : undefined;
	} catch {
		return undefined;
	}
};
