import type { QueryClient } from "@tanstack/solid-query";
import type {
	JsonConsoleProject,
	JsonProject,
	JsonProjectKey,
	JsonProjectKeyCreated,
} from "../../types/bencher";
import type { Api } from "../api";
import { SHELL_KEY, VERSION_KEY, rememberVersion } from "../memory";
import { type Patch, patched } from "./general";
import { listed, revoke } from "./keys";

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

export interface KeyPage {
	keys: JsonProjectKey[];
	/** Every key with this status, which the toggle counts. */
	total: number;
}

// The API's largest page, in name order: past it, the first 255 keys by name.
const PER_PAGE = 255;

const keysKey = (slug: string, revoked: boolean) => [
	"console",
	"keys",
	slug,
	revoked ? "revoked" : "active",
];

export const keysQuery = (api: Api, slug: string, revoked: boolean) => ({
	queryKey: keysKey(slug, revoked),
	queryFn: async ({ signal }: { signal: AbortSignal }): Promise<KeyPage> => {
		const { data, headers } = await api.get<JsonProjectKey[]>(
			`/v0/projects/${encodeURIComponent(slug)}/keys?revoked=${revoked}&per_page=${PER_PAGE}`,
			signal,
		);
		return {
			keys: data,
			total: Number(headers.get("X-Total-Count") ?? data.length),
		};
	},
});

/** A new key leads the active list, without its secret. */
export const addKey = (
	client: QueryClient,
	slug: string,
	created: JsonProjectKeyCreated,
) =>
	client.setQueryData<KeyPage>(keysKey(slug, false), (page) =>
		page
			? {
					keys: [listed(created), ...page.keys],
					total: page.total + 1,
				}
			: page,
	);

/** Show a key revoked at `at` at once; returns the undo. */
export const revokeKey = (
	client: QueryClient,
	slug: string,
	uuid: string,
	at: string,
) => {
	const activeKey = keysKey(slug, false);
	const revokedKey = keysKey(slug, true);
	const active = client.getQueryData<KeyPage>(activeKey);
	const revoked = client.getQueryData<KeyPage>(revokedKey);
	const index = active?.keys.findIndex((key) => key.uuid === uuid) ?? -1;
	const key = active?.keys[index];
	if (active && key) {
		const lists = revoke(
			{ active: active.keys, revoked: revoked?.keys ?? [] },
			uuid,
			at,
		);
		client.setQueryData<KeyPage>(activeKey, {
			keys: lists.active,
			total: active.total - 1,
		});
		if (revoked) {
			client.setQueryData<KeyPage>(revokedKey, {
				keys: lists.revoked,
				total: revoked.total + 1,
			});
		}
	}
	// The undo moves back this key alone: another revoke may have landed since.
	return () => {
		if (!key) {
			return;
		}
		client.setQueryData<KeyPage>(activeKey, (page) => {
			if (!page || page.keys.some((other) => other.uuid === uuid)) {
				return page;
			}
			const keys = [...page.keys];
			keys.splice(index, 0, key);
			return { keys, total: page.total + 1 };
		});
		client.setQueryData<KeyPage>(revokedKey, (page) =>
			page?.keys.some((other) => other.uuid === uuid)
				? {
						keys: page.keys.filter((other) => other.uuid !== uuid),
						total: page.total - 1,
					}
				: page,
		);
	};
};
