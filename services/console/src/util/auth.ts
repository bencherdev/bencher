import { createSignal } from "solid-js";
import type {
	JsonAuthUser,
	JsonOrganizationPermission,
	JsonProjectPermission,
} from "../types/bencher";
import { validUser } from "./valid";
import type { Params } from "./url";
import { BENCHER_API_URL } from "./ext";
import { httpGet } from "./http";

const BENCHER_USER_KEY: string = "BENCHER_USER";

export const defaultUser: JsonAuthUser = {
	user: {
		uuid: "",
		name: "",
		slug: "",
		email: "",
		admin: false,
		locked: true,
	},
	token: "",
};

export const setUser = (user: JsonAuthUser) => {
	if (validUser(user)) {
		window.localStorage.setItem(BENCHER_USER_KEY, JSON.stringify(user));
	} else {
		console.error("Invalid user", user);
	}
};

export const getUser = (): JsonAuthUser => {
	const user = getUserRaw();
	if (validUser(user)) {
		return user;
	} else {
		console.error("Invalid user", user);
		return defaultUser;
	}
};

export const getUserRaw = (): JsonAuthUser => {
	const user_str = window.localStorage.getItem(BENCHER_USER_KEY);
	if (!user_str) {
		return defaultUser;
	}
	return JSON.parse(user_str);
};

export const removeUser = () => {
	window.localStorage.removeItem(BENCHER_USER_KEY);
};

const [authUsr, setAuthUsr] = createSignal<JsonAuthUser>(getUserRaw());
setInterval(() => {
	const usr = authUsr();
	const userRaw = getUserRaw();
	if (usr.toString() !== userRaw.toString()) {
		setAuthUsr(userRaw);
	}
}, 100);

export const authUser = authUsr;

export const isAllowedOrganization = async (
	pathParams: Params,
	permission: JsonOrganizationPermission,
): Promise<boolean> => {
	return is_allowed(
		`${BENCHER_API_URL()}/v0/organizations/${
			pathParams?.organization_slug
		}/allowed/${permission}`,
	);
};

export const isAllowedProject = async (
	pathParams: Params,
	permission: JsonProjectPermission,
): Promise<boolean> => {
	return is_allowed(
		`${BENCHER_API_URL()}/v0/projects/${
			pathParams?.project_slug
		}/allowed/${permission}`,
	);
};

export const is_allowed = async (url: string): Promise<boolean> => {
	const token = authUsr().token;
	// if (!validJwt(token)) {
	// 	return false;
	// }
	return await httpGet(url, token)
		.then((resp) => resp?.data?.allowed)
		.catch((error) => {
			console.error(error);
			return false;
		});
};
