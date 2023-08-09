import { createSignal } from "solid-js";
import {
	JsonProjectPermission,
	type JsonAuthUser,
	JsonOrganizationPermission,
} from "../types/bencher";
import { validUser } from "./valid";
import { BENCHER_API_URL } from "./ext";
import { httpGet } from "./http";
import type { Params } from "astro";

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
	params: Params,
	permission: JsonOrganizationPermission,
): Promise<boolean> => {
	return is_allowed(
		`${BENCHER_API_URL()}/v0/organizations/${
			params?.organization
		}/allowed/${permission}`,
	);
};

export const isAllowedProject = async (
	params: Params,
	permission: JsonProjectPermission,
): Promise<boolean> => {
	return is_allowed(
		`${BENCHER_API_URL()}/v0/projects/${params?.project}/allowed/${permission}`,
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

export const isAllowedOrganizationEdit = (params: Params) =>
	isAllowedOrganization(params, JsonOrganizationPermission.Edit);

export const isAllowedProjectEdit = (params: Params) =>
	isAllowedProject(params, JsonProjectPermission.Edit);
