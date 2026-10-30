import type { QueryClient } from "@tanstack/solid-query";
import type { JsonOAuthGrant } from "../../types/bencher";
import { type Api, ApiError } from "../api";
import { createPaged } from "../list/paged";
import { dropRow, totalOf } from "../list/pages";

const USER_KEY_PREFIX = "bencher_user_";

// A page of apps: each is a card, so a screen holds a few.
const PER_PAGE = 16;

const grantsPath = (user: string) =>
	`/v0/users/${encodeURIComponent(user)}/oauth/grants`;

export const grantPath = (user: string, grant: string) =>
	`${grantsPath(user)}/${grant}`;

export const grantsKey = (user: string) => ["account", "apps", user];

/** The apps connected to the reader's account, newest first, a page at a time. */
export const grantList = (api: Api, user: () => string) =>
	createPaged<JsonOAuthGrant>({
		queryKey: () => grantsKey(user()),
		perPage: PER_PAGE,
		fetchPage: async (page, perPage, signal) => {
			const { data, headers } = await api.get<JsonOAuthGrant[]>(
				`${grantsPath(user())}?per_page=${perPage}&page=${page}&direction=desc`,
				signal,
			);
			return { rows: data, total: totalOf(headers, data) };
		},
	});

/** Show `uuid` gone at once; returns the undo. */
export const dropGrant = (client: QueryClient, user: string, uuid: string) =>
	dropRow<JsonOAuthGrant>(client, grantsKey(user), uuid);

/** Why the API will not list the apps, when no retry would change that. */
export const refusalOf = (error: unknown, token: string) => {
	if (!(error instanceof ApiError)) {
		return undefined;
	}
	switch (error.kind) {
		// The API answers 404 for every user while it does not connect apps.
		case "not_found":
			return "Not available on this server";
		case "forbidden":
			return token.startsWith(USER_KEY_PREFIX)
				? "Not available with an API key"
				: "No access";
		default:
			return undefined;
	}
};

const dayFormat = new Intl.DateTimeFormat("en-US", {
	month: "short",
	day: "numeric",
	year: "numeric",
});

/** "Oct 2, 2026", as Keys dates a key. */
export const day = (at: string) => dayFormat.format(Date.parse(at));

/** When the app last renewed its access, if it ever has. */
export const lastRefreshed = (
	grant: Pick<JsonOAuthGrant, "created" | "modified">,
): string | undefined =>
	Date.parse(grant.modified) > Date.parse(grant.created)
		? grant.modified
		: undefined;
