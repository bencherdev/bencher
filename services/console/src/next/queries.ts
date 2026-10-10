import type { QueryClient } from "@tanstack/solid-query";
import { createSignal } from "solid-js";
import type { JsonConsoleProject } from "../types/bencher";
import type { Api } from "./api";
import type { BmfVersion } from "./memory";

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
				`/v0/projects/${encodeURIComponent(slug)}/console`,
				signal,
			)
		).data,
});

/**
 * Each project's BMF version by slug, from responses fetched on this page load
 * only: a response restored from the cache can predate a move between versions.
 */
export const fetchedVersions = (client: QueryClient) => {
	const [versions, setVersions] = createSignal<Record<string, BmfVersion>>({});
	client.getQueryCache().subscribe((event) => {
		const [scope, kind, slug] = event.query.queryKey;
		if (
			event.type !== "updated" ||
			event.action.type !== "success" ||
			scope !== "console" ||
			kind !== "project" ||
			typeof slug !== "string"
		) {
			return;
		}
		const project = event.action.data as JsonConsoleProject | undefined;
		const version = project?.project.bmf_version;
		if (version === 0 || version === 1) {
			setVersions((all) => ({ ...all, [slug]: version }));
		}
	});
	return versions;
};
