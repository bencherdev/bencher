import type { JsonPlanCheckout } from "../../types/bencher";
import { type Api, ApiError } from "../api";

/** What Billing shows of Stripe's return, from a checkout that sells only Pro. */
export interface Enrollment {
	status: "enrolling" | "enrolled" | "failed";
	error?: unknown;
}

/** The checkout session Stripe's return to Billing names, for the API to record and check, or nothing for any other link. */
export const checkoutReturn = (
	search: string,
): JsonPlanCheckout | undefined => {
	const checkout = new URLSearchParams(search).get("checkout");
	return checkout ? { checkout } : undefined;
};

const CONFLICT = 409;
// The API's one conflict for an organization that already holds a plan,
// which a replayed return meets once its plan is recorded.
const HAS_PLAN = "Organization already has a plan";

const hasPlan = (error: unknown) => {
	if (!(error instanceof ApiError && error.status === CONFLICT)) {
		return false;
	}
	try {
		return JSON.parse(error.message)?.message?.endsWith(HAS_PLAN) === true;
	} catch {
		return false;
	}
};

/** Record the plan the return names, then `reread` the usage, so the outcome shows with the plan it bought. */
export const recordCheckout = async (
	api: Api,
	organization: string,
	search: string,
	reread: () => Promise<void>,
	show: (enrollment: Enrollment | undefined) => void,
) => {
	const plan = checkoutReturn(search);
	if (!plan) {
		show(undefined);
		return;
	}
	try {
		await api.send("POST", `/v0/organizations/${organization}/plan`, plan);
	} catch (error) {
		if (!hasPlan(error)) {
			show({ status: "failed", error });
			return;
		}
	}
	await reread();
	show({ status: "enrolled" });
};
