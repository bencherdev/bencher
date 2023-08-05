import type { JsonAuthUser } from "../types/bencher";
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
};

export const setUser = (user: JsonAuthUser) => {
    if (validUser(user)) {
        window.localStorage.setItem(BENCHER_USER_KEY, JSON.stringify(user));
    } else {
        console.error("Invalid user", user);
    }
}

export const getUser = (): JsonAuthUser => {
    const user_str = window.localStorage.getItem(BENCHER_USER_KEY);
    if (!user_str) {
        return defaultUser;
    }
    const user = JSON.parse(user_str);
	if (validUser(user)) {
		return user;
	} else {
        console.error("Invalid user", user);
		return defaultUser;
	}
}

export const removeUser = () => {
    window.localStorage.removeItem(BENCHER_USER_KEY);
}