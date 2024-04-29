import type { PlanLevel } from "../../types/bencher";
import { validEmail, validJwt, validUserName } from "../../util/valid";
import { EMAIL, USERNAME } from "./authFields";

export const PLAN_PARAM = "plan";
export const INVITE_PARAM = "invite";
export const EMAIL_PARAM = "email";
export const TOKEN_PARAM = "token";

export const planParam = (plan: undefined | PlanLevel) =>
	plan ? `?${PLAN_PARAM}=${plan}` : "";

export const AUTH_FIELDS = {
	username: {
		...USERNAME,
		validate: validUserName,
	},
	email: {
		...EMAIL,
		validate: validEmail,
	},
	consent: {
		label: "I Agree",
		type: "checkbox",
		placeholder: (
			<small>
				{" "}
				I agree to the{" "}
				{
					<a href="/legal/terms-of-use" target="_blank">
						terms of use
					</a>
				}
				,{" "}
				{
					<a href="/legal/privacy" target="_blank">
						privacy policy
					</a>
				}
				, and{" "}
				{
					<a href="/legal/license" target="_blank">
						license agreement
					</a>
				}
				.
			</small>
		),
	},
	token: {
		label: "Token",
		type: "text",
		placeholder: "jwt_header.jwt_payload.jwt_verify_signature",
		icon: "fas fa-key",
		help: "Must be a valid JWT (JSON Web Token)",
		validate: validJwt,
	},
};
