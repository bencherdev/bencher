import type { JsonConsoleProject, JsonConsoleUser } from "../types/bencher";
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
