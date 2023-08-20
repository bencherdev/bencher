import { createSignal } from "solid-js";
import {
	JsonProjectPermission,
	type JsonAuthUser,
	JsonOrganizationPermission,
} from "../types/bencher";
import { validUser } from "./valid";
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

export const setUser = (user: JsonAuthUser): boolean => {
	if (validUser(user)) {
		window.localStorage.setItem(BENCHER_USER_KEY, JSON.stringify(user));
		return true;
	} else {
		console.error("Invalid user", user);
		return false;
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
	apiUrl: string,
	params: undefined | Params,
	permission: JsonOrganizationPermission,
): Promise<boolean> => {
	if (!params?.organization) {
		return false;
	}
	return isAllowed(
		apiUrl,
		`/v0/organizations/${params.organization}/allowed/${permission}`,
	);
};

export const isAllowedProject = async (
	apiUrl: string,
	params: undefined | Params,
	permission: JsonProjectPermission,
): Promise<boolean> => {
	if (!params?.project) {
		return false;
	}
	return isAllowed(
		apiUrl,
		`/v0/projects/${params.project}/allowed/${permission}`,
	);
};

export const isAllowed = async (
	hostname: string,
	pathname: string,
): Promise<boolean> => {
	const token = authUsr().token;
	if (!token) {
		return false;
	}
	return await httpGet(hostname, pathname, token)
		.then((resp) => resp?.data?.allowed)
		.catch((error) => {
			console.error(error);
			return false;
		});
};

export const isAllowedOrganizationEdit = (apiUrl: string, params: Params) =>
	isAllowedOrganization(apiUrl, params, JsonOrganizationPermission.Edit);

export const isAllowedProjectEdit = (apiUrl: string, params: Params) =>
	isAllowedProject(apiUrl, params, JsonProjectPermission.Edit);

export const isAllowedProjectDelete = (apiUrl: string, params: Params) =>
	isAllowedProject(apiUrl, params, JsonProjectPermission.Delete);
