import { planParam } from "../../components/auth/auth";
import { signedInPath } from "../../components/auth/back";
import type { JsonSignedIn } from "../../types/bencher";
import { rememberReader } from "../reader";
import { forgetPending } from "./pending";

/**
 * Where a finished sign-in goes: an app's pending consent, then the page the
 * reader asked for, then the console, which lands a new account on onboarding
 * and anyone else on their main project.
 */
export const landing = (
	{ authorization, plan }: Pick<JsonSignedIn, "authorization" | "plan">,
	kept: (authorization: unknown) => string | undefined = signedInPath,
) => kept(authorization) ?? `/console${planParam(plan)}`;

/** Where the console kept the signed in user and its session token before the cookie. */
const LEGACY_USER_KEY = "BENCHER_USER";

/**
 * Keep the reader's hint in place of any user and session token stored before
 * the cookie, forget this tab's sign-in, and go where a finished sign-in goes.
 */
export const finish = (
	signedIn: JsonSignedIn,
	{
		local = localStorage,
		session = sessionStorage,
		go = (to: string) => location.replace(to),
		kept = signedInPath,
	}: {
		local?: Pick<Storage, "setItem" | "removeItem">;
		session?: Pick<Storage, "getItem" | "setItem" | "removeItem">;
		go?: (to: string) => void;
		kept?: (authorization: unknown) => string | undefined;
	} = {},
) => {
	local.removeItem(LEGACY_USER_KEY);
	rememberReader(local, signedIn.user);
	forgetPending(session);
	go(landing(signedIn, kept));
};
