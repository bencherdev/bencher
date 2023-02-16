import { BENCHER_API_URL } from "../../site/util";
import { Auth, FormKind } from "./types";

const AUTH_CONFIG = {
	[Auth.SIGNUP]: {
		auth: Auth.SIGNUP,
		title: "Sign up",
		form: {
			kind: FormKind.SIGNUP,
			token: true,
			redirect: "/auth/confirm",
			notification: {
				success: "Sign up successful",
				danger: "Sign up failed",
			},
		},
	},
	[Auth.LOGIN]: {
		auth: Auth.LOGIN,
		title: "Log in",
		form: {
			kind: FormKind.LOGIN,
			token: true,
			redirect: "/auth/confirm",
			notification: {
				success: "Log in successful",
				danger: "Log in failed",
			},
		},
	},
	[Auth.CONFIRM]: {
		auth: Auth.CONFIRM,
		title: "Confirm Token",
		sub: "Please check your email for a one-time token. Either click the link or paste the token here.",
		form: {
			path: `${BENCHER_API_URL()}/v0/auth/confirm`,
			redirect: {
				free: "/console",
				team: "/console/billing",
				enterprise: "/console/billing",
			},
		},
	},
	[Auth.LOGOUT]: {
		auth: Auth.LOGOUT,
		title: "Log out",
		redirect: "/auth/login",
	},
};

export default AUTH_CONFIG;
