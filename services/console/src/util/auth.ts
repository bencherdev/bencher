import { createSignal } from "solid-js";
import {
	ProjectPermission,
	type JsonAuthUser,
	OrganizationPermission,
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
	}
	console.error("Invalid user", user);
	return false;
};

export const getUser = (): JsonAuthUser => {
	const user = getUserRaw();
	if (validUser(user)) {
		return user;
	}
	console.error("Invalid user", user);
	return defaultUser;
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
	permission: OrganizationPermission,
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
	permission: ProjectPermission,
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
	isAllowedOrganization(apiUrl, params, OrganizationPermission.Edit);

export const isAllowedOrganizationManage = (apiUrl: string, params: Params) =>
	isAllowedOrganization(apiUrl, params, OrganizationPermission.Manage);

export const isAllowedOrganizationDeleteRole = (
	apiUrl: string,
	params: Params,
) => isAllowedOrganization(apiUrl, params, OrganizationPermission.DeleteRole);

export const isAllowedProjectEdit = (apiUrl: string, params: Params) =>
	isAllowedProject(apiUrl, params, ProjectPermission.Edit);

export const isAllowedProjectDelete = (apiUrl: string, params: Params) =>
	isAllowedProject(apiUrl, params, ProjectPermission.Delete);

export const isAllowedProjectManage = (apiUrl: string, params: Params) =>
	isAllowedProject(apiUrl, params, ProjectPermission.Manage);

export const isSameUser = (_apiUrl: string, params: Params) =>
	params?.user === authUsr().user.uuid ||
	params?.user === authUsr().user.slug ||
	authUsr().user.admin;
