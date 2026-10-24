import * as Sentry from "@sentry/astro";
import { createRoot, createSignal, onCleanup } from "solid-js";
import type { JsonAuthUser } from "../types/bencher";
import { dateTimeMillis } from "./convert";
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
