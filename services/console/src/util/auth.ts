import * as Sentry from "@sentry/astro";
import type { Params } from "astro";
import { createRoot, createSignal, onCleanup } from "solid-js";
import {
	type JsonAuthUser,
	OrganizationPermission,
	ProjectPermission,
} from "../types/bencher";
import { dateTimeMillis } from "./convert";
import { httpGet } from "./http";
import { removeOrganization } from "./organization";
import { validUser } from "./valid";

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
	creation: "",
	expiration: "",
};

export const setUser = (user: JsonAuthUser): boolean => {
	if (validUser(user)) {
		localStorage.setItem(BENCHER_USER_KEY, JSON.stringify(user));
		return true;
	}
	const err = `Invalid user: ${JSON.stringify(user)}`;
	console.error(err);
	Sentry.captureMessage(err);
	return false;
};

export const getUser = (): JsonAuthUser => {
	const user = getUserRaw();
	if (validUser(user)) {
		return user;
	}
	const err = `Invalid user: ${JSON.stringify(user)}`;
	console.error(err);
	Sentry.captureMessage(err);
	return defaultUser;
};

export const getUserRaw = (): JsonAuthUser => {
	const user_str = localStorage.getItem(BENCHER_USER_KEY);
	if (!user_str) {
		return defaultUser;
	}
	return JSON.parse(user_str);
};

export const removeUser = () => {
	localStorage.removeItem(BENCHER_USER_KEY);
};

export const authUser = createRoot(() => {
	const [authUser, setAuthUser] = createSignal<JsonAuthUser>(getUserRaw());
	const interval = setInterval(() => {
		const usr = authUser();
		const userRaw = getUserRaw();
		if (usr.toString() !== userRaw.toString()) {
			setAuthUser(userRaw);
		} else if (
			userRaw?.token &&
			(dateTimeMillis(userRaw?.expiration) ?? 0) < Date.now()
		) {
			removeUser();
			removeOrganization();
			setAuthUser(defaultUser);
		}
	}, 100);

	onCleanup(() => clearInterval(interval));

	return authUser;
});

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
	const token = authUser().token;
	if (!token) {
		return false;
	}
	return await httpGet(hostname, pathname, token)
		.then((resp) => resp?.data?.allowed)
		.catch((error) => {
			console.error(error);
			Sentry.captureException(error);
			return false;
		});
};

export const isAllowedOrganizationCreate = (apiUrl: string, params: Params) =>
	isAllowedOrganization(apiUrl, params, OrganizationPermission.Create);

export const isAllowedOrganizationEdit = (apiUrl: string, params: Params) =>
	isAllowedOrganization(apiUrl, params, OrganizationPermission.Edit);

export const isAllowedOrganizationDelete = (apiUrl: string, params: Params) =>
	isAllowedOrganization(apiUrl, params, OrganizationPermission.Delete);

export const isAllowedOrganizationManage = (apiUrl: string, params: Params) =>
	isAllowedOrganization(apiUrl, params, OrganizationPermission.Manage);

export const isAllowedOrganizationDeleteRole = (
	apiUrl: string,
	params: Params,
) => isAllowedOrganization(apiUrl, params, OrganizationPermission.DeleteRole);

export const isAllowedProjectCreate = (apiUrl: string, params: Params) =>
	isAllowedProject(apiUrl, params, ProjectPermission.Create);

export const isAllowedProjectEdit = (apiUrl: string, params: Params) =>
	isAllowedProject(apiUrl, params, ProjectPermission.Edit);

export const isAllowedProjectDelete = (apiUrl: string, params: Params) =>
	isAllowedProject(apiUrl, params, ProjectPermission.Delete);

export const isAllowedProjectManage = (apiUrl: string, params: Params) =>
	isAllowedProject(apiUrl, params, ProjectPermission.Manage);

export const isSameUser = (_apiUrl: string, params: Params) =>
	params?.user === authUser().user.uuid ||
	params?.user === authUser().user.slug ||
	authUser().user.admin;
