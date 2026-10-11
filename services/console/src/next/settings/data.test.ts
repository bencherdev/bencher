import { QueryClient } from "@tanstack/solid-query";
import { describe, expect, test } from "vitest";
import {
	type JsonConsoleProject,
	type JsonProject,
	type JsonProjectKey,
	Visibility,
} from "../../types/bencher";
import {
	VERSION_KEY,
	readShell,
	rememberShell,
	rememberedVersion,
} from "../memory";
import {
	type KeyPage,
	addKey,
	applyPatch,
	forgetSlug,
	revokeKey,
	settleProject,
} from "./data";

const project: JsonProject = {
	uuid: "p",
	organization: "o",
	name: "Hashbrown",
	slug: "hashbrown",
	url: "https://github.com/pompeii-llc/hashbrown",
	visibility: Visibility.Public,
	bmf_version: 1,
	created: "2026-09-01T00:00:00Z",
	modified: "2026-09-01T00:00:00Z",
};

const bootstrap: JsonConsoleProject = {
	project,
	organization: { uuid: "o", name: "Pompeii LLC", slug: "pompeii-llc" },
	permissions: {
		view: true,
		create: true,
		edit: true,
		delete: true,
		manage: true,
	},
	active_alerts: 1,
};

const storage = () => {
	const map = new Map<string, string>();
	return {
		getItem: (key: string) => map.get(key) ?? null,
		setItem: (key: string, value: string) => {
			map.set(key, value);
		},
		removeItem: (key: string) => {
			map.delete(key);
		},
	};
};

const shell = (client: QueryClient, slug: string) =>
	client.getQueryData<JsonConsoleProject>(["console", "project", slug]);

describe("a save", () => {
	// Kills a rename that waits for the API before the shell shows it, a slug
	// shown before the API takes it, and an undo that leaves the refused change
	// behind.
	test("shows the change at once and the undo puts the project back", () => {
		const client = new QueryClient();
		client.setQueryData(["console", "project", "hashbrown"], bootstrap);

		const undo = applyPatch(client, "hashbrown", {
			name: "Hash Browns",
			url: null,
			slug: "hash-browns",
		});
		expect(shell(client, "hashbrown")?.project.name).toBe("Hash Browns");
		// A slug moves only once the API agrees, since links and runs name it.
		expect(shell(client, "hashbrown")?.project.slug).toBe("hashbrown");
		expect(shell(client, "hashbrown")?.project.url).toBeUndefined();
		expect(shell(client, "hashbrown")?.permissions).toEqual(
			bootstrap.permissions,
		);

		undo();
		expect(shell(client, "hashbrown")).toEqual(bootstrap);
	});

	// Kills a slug change that leaves the new page to load from nothing, or
	// without its version for a classic link.
	test("that moves the slug holds the answer under the new slug, with its version", () => {
		const client = new QueryClient();
		client.setQueryData(["console", "project", "hashbrown"], bootstrap);
		const store = storage();
		const moved = { ...project, slug: "hash-browns" };

		settleProject(client, store, "hashbrown", moved);
		expect(shell(client, "hash-browns")).toEqual({
			...bootstrap,
			project: moved,
		});
		expect(rememberedVersion(store, "hash-browns")).toBe(1);
	});
});

describe("forgetSlug", () => {
	// Kills a slug no project has left answering from the cache, or from the
	// memories a page paints first, and a forget that takes other projects too.
	test("drops the slug's cache and memories, and only the slug's", () => {
		const client = new QueryClient();
		client.setQueryData(["console", "project", "hashbrown"], bootstrap);
		client.setQueryData(["console", "keys", "hashbrown", "active"], {
			keys: [],
			total: 0,
		});
		client.setQueryData(["console", "project", "tater-tot"], bootstrap);
		const store = storage();
		store.setItem(
			VERSION_KEY,
			JSON.stringify({ hashbrown: 1, "tater-tot": 1 }),
		);
		const shellMemory = {
			organization: "Pompeii LLC",
			organizationUuid: "o",
			name: "Hashbrown",
			alerts: 1,
		};
		rememberShell(store, "reader", "hashbrown", shellMemory);
		rememberShell(store, "reader", "tater-tot", shellMemory);

		forgetSlug(client, store, "hashbrown");
		expect(shell(client, "hashbrown")).toBeUndefined();
		expect(
			client.getQueryData(["console", "keys", "hashbrown", "active"]),
		).toBeUndefined();
		expect(rememberedVersion(store, "hashbrown")).toBeUndefined();
		expect(readShell(store, "reader", "hashbrown")).toBeUndefined();

		expect(shell(client, "tater-tot")).toEqual(bootstrap);
		expect(rememberedVersion(store, "tater-tot")).toBe(1);
		expect(readShell(store, "reader", "tater-tot")).toEqual(shellMemory);
	});
});

const key = (name: string, revoked?: string): JsonProjectKey => ({
	uuid: `uuid-${name}`,
	project: "p",
	name,
	creation: "2026-09-02T00:00:00Z",
	expiration: "2026-10-02T00:00:00Z",
	...(revoked ? { revoked } : {}),
});

const keys = (client: QueryClient, status: "active" | "revoked") =>
	client.getQueryData<KeyPage>(["console", "keys", "hashbrown", status]);

describe("keys", () => {
	// Kills a revoke that waits for the API, miscounts the toggle, or whose
	// undo loses a key.
	test("a revoke moves the key at once, and the undo moves it back", () => {
		const client = new QueryClient();
		const active = { keys: [key("a"), key("b")], total: 2 };
		const revoked = { keys: [key("old", "2026-08-04T00:00:00Z")], total: 1 };
		client.setQueryData(["console", "keys", "hashbrown", "active"], active);
		client.setQueryData(["console", "keys", "hashbrown", "revoked"], revoked);

		const undo = revokeKey(
			client,
			"hashbrown",
			"uuid-b",
			"2026-09-13T00:00:00Z",
		);
		expect(keys(client, "active")).toEqual({ keys: [key("a")], total: 1 });
		expect(keys(client, "revoked")).toEqual({
			keys: [
				key("b", "2026-09-13T00:00:00Z"),
				key("old", "2026-08-04T00:00:00Z"),
			],
			total: 2,
		});

		undo();
		expect(keys(client, "active")).toEqual(active);
		expect(keys(client, "revoked")).toEqual(revoked);
	});

	// Kills an undo that puts back a whole list, which brings back a key whose
	// own revoke the API accepted meanwhile.
	test("the undo of one revoke leaves another standing", () => {
		const client = new QueryClient();
		client.setQueryData(["console", "keys", "hashbrown", "active"], {
			keys: [key("alpha"), key("bravo")],
			total: 2,
		});
		client.setQueryData(["console", "keys", "hashbrown", "revoked"], {
			keys: [],
			total: 0,
		});

		const undoAlpha = revokeKey(
			client,
			"hashbrown",
			"uuid-alpha",
			"2026-09-13T00:00:00Z",
		);
		revokeKey(client, "hashbrown", "uuid-bravo", "2026-09-13T00:01:00Z");
		undoAlpha();
		expect(keys(client, "active")).toEqual({ keys: [key("alpha")], total: 1 });
		expect(keys(client, "revoked")).toEqual({
			keys: [key("bravo", "2026-09-13T00:01:00Z")],
			total: 1,
		});
	});

	// Kills a new key missing from the list until a refetch, or kept with its secret.
	test("a new key leads the active list without its secret", () => {
		const client = new QueryClient();
		client.setQueryData(["console", "keys", "hashbrown", "active"], {
			keys: [key("a")],
			total: 1,
		});
		addKey(client, "hashbrown", {
			uuid: "uuid-new",
			project: "p",
			name: "new",
			key: "bencher_run_secret",
			creation: "2026-09-13T00:00:00Z",
			expiration: "2026-12-12T00:00:00Z",
		});
		const page = keys(client, "active");
		expect(page?.total).toBe(2);
		expect(page?.keys.map(({ name }) => name)).toEqual(["new", "a"]);
		expect(JSON.stringify(page)).not.toContain("bencher_run_secret");
	});
});
