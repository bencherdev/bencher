import type { KeyHome } from "../settings/data";

const userPath = (user: string) => `/v0/users/${encodeURIComponent(user)}`;

export const userKeys = (user: string): KeyHome => ({
	path: `${userPath(user)}/keys`,
	queryKey: ["account", "keys", user],
});

/** The legacy tokens a user has not revoked, which the API no longer makes. */
export const legacyTokenList = (user: string): KeyHome => ({
	path: `${userPath(user)}/tokens`,
	queryKey: ["account", "legacy", user],
});
