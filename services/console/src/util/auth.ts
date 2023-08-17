import { createResource, createSignal } from "solid-js";
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

export const [authUser, { refetch }] = createResource(true, async () =>
	getUserRaw(),
);
setInterval(() => {
	refetch();
}, 100);

export const isAllowedOrganization = async (
	params: undefined | Params,
	permission: JsonOrganizationPermission,
): Promise<boolean> => {
	if (!params?.organization) {
		return false;
	}
	return isAllowed(
		`${BENCHER_API_URL()}/v0/organizations/${
			params.organization
		}/allowed/${permission}`,
	);
};

export const isAllowedProject = async (
	params: undefined | Params,
	permission: JsonProjectPermission,
): Promise<boolean> => {
	if (!params?.project) {
		return false;
	}
	return isAllowed(
		`${BENCHER_API_URL()}/v0/projects/${params.project}/allowed/${permission}`,
	);
};

export const isAllowed = async (url: string): Promise<boolean> => {
	const token = authUsr().token;
	if (!token) {
		return false;
	}
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

export const isAllowedProjectDelete = (params: Params) =>
	isAllowedProject(params, JsonProjectPermission.Delete);
