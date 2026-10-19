import type { QueryClient } from "@tanstack/solid-query";
import type { JsonOAuthGrant } from "../../types/bencher";
import { type Api, ApiError } from "../api";

const USER_KEY_PREFIX = "bencher_user_";

// The most the API lists at once.
const PER_PAGE = 255;

export interface GrantPage {
	/** Newest first. */
	grants: JsonOAuthGrant[];
	/** Every app the reader has connected, which the list may stop short of. */
	total: number;
}

const grantsPath = (user: string) =>
	`/v0/users/${encodeURIComponent(user)}/oauth/grants`;

export const grantPath = (user: string, grant: string) =>
	`${grantsPath(user)}/${grant}`;

export const grantsQuery = (api: Api, user: string) => ({
	queryKey: ["account", "apps", user],
	queryFn: async ({ signal }: { signal: AbortSignal }): Promise<GrantPage> => {
		const { data, headers } = await api.get<JsonOAuthGrant[]>(
			`${grantsPath(user)}?per_page=${PER_PAGE}&direction=desc`,
			signal,
		);
		return {
			grants: data,
			total: Number(headers.get("X-Total-Count") ?? data.length),
		};
	},
});

/** Show `uuid` gone at once; returns the undo. */
export const dropGrant = (
	client: QueryClient,
	api: Api,
	user: string,
	uuid: string,
) => {
	const { queryKey } = grantsQuery(api, user);
	const before = client.getQueryData<GrantPage>(queryKey);
	const index = before?.grants.findIndex((grant) => grant.uuid === uuid) ?? -1;
	const grant = before?.grants[index];
	if (!(before && grant)) {
		return () => {};
	}
	client.setQueryData<GrantPage>(queryKey, {
		grants: before.grants.filter((other) => other.uuid !== uuid),
		total: before.total - 1,
	});
	// The undo puts back this app alone: another revoke may have landed since.
	return () =>
		client.setQueryData<GrantPage>(queryKey, (page) =>
			!page || page.grants.some((other) => other.uuid === uuid)
				? page
				: {
						grants: [
							...page.grants.slice(0, index),
							grant,
							...page.grants.slice(index),
						],
						total: page.total + 1,
					},
		);
};

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
