import { requestUuid } from "../../components/auth/authorize/path";
import type { Carry } from "./pending";

const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;
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
