import type { PlanLevel } from "../../types/bencher";

export const PLAN_PARAM = "plan";
export const INVITE_PARAM = "invite";
export const TOKEN_PARAM = "token";
export const USER_PARAM = "user";
export const CLAIM_PARAM = "claim";

export const planParam = (plan: undefined | PlanLevel) =>
	plan ? `?${PLAN_PARAM}=${plan}` : "";
