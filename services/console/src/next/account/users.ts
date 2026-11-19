import type { JsonUser } from "../../types/bencher";
import type { Api } from "../api";
import { createPaged } from "../list/paged";
import { totalOf } from "../list/pages";
import { screenBatch } from "../list/rows";
import { formatCount } from "../plot/count";

/** Every user on the server, or those whose name, slug, or UUID holds `search`, in name order. */
export const usersKey = (search: string) => ["server", "users", search];

export const usersPath = (search: string, page: number, perPage: number) => {
	const query = new URLSearchParams({
		per_page: String(perPage),
		page: String(page),
	});
	if (search) {
		query.set("search", search);
	}
	return `/v0/users?${query}`;
};

/** The server's users, a screen's worth at a time. */
export const userList = (api: Api, search: () => string) =>
	createPaged<JsonUser>({
		queryKey: () => usersKey(search()),
		perPage: screenBatch(),
		keep: true,
		fetchPage: async (page, perPage, signal) => {
			const { data, headers } = await api.get<JsonUser[]>(
				usersPath(search(), page, perPage),
				signal,
			);
			return { rows: data, total: totalOf(headers, data) };
		},
	});

export const userCount = (count: number) =>
	`${formatCount(count)} ${count === 1 ? "user" : "users"}`;
