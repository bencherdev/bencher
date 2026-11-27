import type {
	JsonConsoleProject,
	JsonConsoleUser,
	JsonUser,
} from "../types/bencher";
import type { Api } from "./api";

/**
 * Everything the shell shows for a project, in the one request it makes: the
 * project, its organization, the reader's permissions, and the active alerts.
 * It starts at boot, before the cache is restored.
 */
export const consoleProjectQuery = (api: Api, slug: string) => ({
	queryKey: ["console", "project", slug],
	queryFn: async ({ signal }: { signal: AbortSignal }) =>
		(
			await api.get<JsonConsoleProject>(
				`/v0/console/projects/${encodeURIComponent(slug)}`,
				signal,
			)
		).data,
});

/** The reader's organizations and every project in them, which the switcher lists. */
export const consoleUserQuery = (api: Api, reader: string) => ({
	queryKey: ["console", "user", reader],
	queryFn: async ({ signal }: { signal: AbortSignal }) =>
		(
			await api.get<JsonConsoleUser>(
				`/v0/console/users/${encodeURIComponent(reader)}`,
				signal,
			)
		).data,
});

/**
 * A user's account, by slug or UUID: the email Profile edits, and the one the
 * account menu and Feedback show, since the reader's hint holds none.
 */
export const userQuery = (api: Api, user: string) => ({
	queryKey: ["account", "user", user],
	queryFn: async ({ signal }: { signal: AbortSignal }) =>
		(await api.get<JsonUser>(`/v0/users/${encodeURIComponent(user)}`, signal))
			.data,
});
