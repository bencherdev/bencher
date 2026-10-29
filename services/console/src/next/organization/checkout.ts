import type { JsonNewPlan } from "../../types/bencher";
import { type Api, ApiError } from "../api";
import { planLevel } from "./billing";

/** What Billing shows of Stripe's return. */
export interface Enrollment {
	status: "enrolling" | "enrolled" | "failed";
	level: string;
	error?: unknown;
}

/** The plan Stripe's return to Billing names, for the API to record and check, or nothing for any other link or level. */
export const checkoutReturn = (search: string): JsonNewPlan | undefined => {
	const params = new URLSearchParams(search);
	const checkout = params.get("checkout");
	const level = planLevel(params.get("level"));
	if (!(checkout && level)) {
		return undefined;
	}
	const entitlements = params.get("entitlements");
	const selfHosted = params.get("self_hosted");
	return {
		checkout,
		level,
		...(entitlements === null ? {} : { entitlements: Number(entitlements) }),
		...(selfHosted === null ? {} : { self_hosted: selfHosted }),
	};
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
			show({ status: "failed", level: plan.level, error });
			return;
		}
	}
	await reread();
	show({ status: "enrolled", level: plan.level });
};
