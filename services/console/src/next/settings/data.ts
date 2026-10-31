import type { QueryClient } from "@tanstack/solid-query";
import type {
	JsonConsoleProject,
	JsonConsoleUserProject,
	JsonProject,
} from "../../types/bencher";
import type { Api } from "../api";
import { projectAt } from "../home";
import { SHELL_KEY, VERSION_KEY, rememberVersion } from "../memory";
import { ORGANIZATIONS_PATH } from "../paths";
import { type Patch, patched } from "./general";
import { dropRow, findRow, pageQuery, putRow, totalOf } from "../list/pages";
import { screenBatch } from "../list/rows";
import { type KeyRow, listed } from "./keys";

// Console query keys put the project's slug third: ["console", kind, slug, ...].
const projectKey = (slug: string) => ["console", "project", slug];

/** Show `patch` in the shell and the form at once, all but a new slug; returns the undo. */
export const applyPatch = (client: QueryClient, slug: string, patch: Patch) => {
	const key = projectKey(slug);
	const before = client.getQueryData<JsonConsoleProject>(key);
	if (before) {
		const { slug: _moving, ...shown } = patch;
		client.setQueryData<JsonConsoleProject>(key, {
			...before,
			project: patched(before.project, shown),
		});
	}
	return () => client.setQueryData(key, before);
};

type Memory = Pick<Storage, "getItem" | "setItem" | "removeItem">;

/**
 * Hold the API's answer under the project's slug, which a save can change:
 * the shell then paints the new slug's page at once, and a classic link to
 * the new slug finds its version.
 */
export const settleProject = (
	client: QueryClient,
	storage: Memory,
	from: string,
	project: JsonProject,
) => {
	const before = client.getQueryData<JsonConsoleProject>(projectKey(from));
	if (before) {
		client.setQueryData<JsonConsoleProject>(projectKey(project.slug), {
			...before,
			project,
		});
	}
	const version = project.bmf_version;
	if (project.slug !== from && (version === 0 || version === 1)) {
		rememberVersion(storage, project.slug, version);
	}
};

/** Drop everything cached and remembered under a slug no project has any more. */
export const forgetSlug = (
	client: QueryClient,
	storage: Memory,
	slug: string,
) => {
	client.removeQueries({
		predicate: ({ queryKey }) =>
			queryKey[0] === "console" && queryKey[2] === slug,
	});
	// The version memory maps slugs to versions, the shell memory's projects to names.
	for (const key of [VERSION_KEY, SHELL_KEY]) {
		try {
			const memory = JSON.parse(storage.getItem(key) ?? "null");
			const bySlug = key === SHELL_KEY ? memory?.projects : memory;
			if (bySlug && typeof bySlug === "object" && slug in bySlug) {
				delete bySlug[slug];
				storage.setItem(key, JSON.stringify(memory));
			}
		} catch {
			// A memory the browser refuses, or cannot read, stays as it is.
		}
	}
};

/** The reader's project with the newest run, as the switcher orders them, where it opens; Organizations with none. */
export const newestHome = (projects: readonly JsonConsoleUserProject[]) => {
	const [newest] = [...projects].sort(
		(a, b) =>
			(b.last_report ?? 0) - (a.last_report ?? 0) ||
			a.name.localeCompare(b.name),
	);
	return (
		(newest && projectAt(newest.slug, newest.bmf_version)) ?? ORGANIZATIONS_PATH
	);
};

/** Where a list of keys lives: its API path, and its place in the cache. */
export interface KeyHome {
	path: string;
	queryKey: readonly string[];
}

export const projectKeys = (slug: string): KeyHome => ({
	path: `/v0/projects/${encodeURIComponent(slug)}/keys`,
	queryKey: ["console", "keys", slug],
});

export const keysKey = (home: KeyHome, revoked: boolean) => [
	...home.queryKey,
	revoked ? "revoked" : "active",
];

/** One page of the keys with a status, newest first, without a legacy token's secret. */
export const keysPage =
	(api: Api, home: KeyHome, revoked: boolean) =>
	async (page: number, perPage: number, signal: AbortSignal) => {
		const { data, headers } = await api.get<KeyRow[]>(
			`${home.path}?revoked=${revoked}&sort=created&direction=desc&per_page=${perPage}&page=${page}`,
			signal,
		);
		return { rows: data.map(listed), total: totalOf(headers, data) };
	};

/** The first page `keyList` asks for, which a visit starts before the list draws. */
export const firstKeysQuery = (api: Api, home: KeyHome, revoked: boolean) =>
	pageQuery(
		keysKey(home, revoked),
		screenBatch(),
		1,
		keysPage(api, home, revoked),
	);

/** A new key leads the active list, without its secret. */
export const addKey = (
	client: QueryClient,
	home: KeyHome,
	created: KeyRow & { key: string },
) => putRow(client, keysKey(home, false), listed(created));

/** Show a key revoked at `at` at once; returns the undo. */
export const revokeKey = (
	client: QueryClient,
	home: KeyHome,
	uuid: string,
	at: string,
) => {
	const activeKey = keysKey(home, false);
	const revokedKey = keysKey(home, true);
	const key = findRow<KeyRow>(client, activeKey, uuid)?.row;
	const undo = dropRow<KeyRow>(client, activeKey, uuid);
	if (key) {
		putRow(client, revokedKey, { ...key, revoked: at });
	}
	// The undo moves back this key alone: another revoke may have landed since.
	return () => {
		if (key) {
			undo();
			dropRow<KeyRow>(client, revokedKey, uuid);
		}
	};
};
