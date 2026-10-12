import { describe, expect, test, vi } from "vitest";
import { ApiError } from "./api";
import {
	type CacheStore,
	MAX_AGE,
	type Persisted,
	clearCache,
	createQueryClient,
	persistCache,
	purgeOtherReaders,
	restoreCache,
} from "./cache";

const memoryStore = () => {
	const entries = new Map<string, Persisted>();
	const store: CacheStore = {
		get: async (key) => entries.get(key),
		set: async (key, value) => {
			entries.set(key, value);
		},
		keys: async () => [...entries.keys()],
		delete: async (key) => {
			entries.delete(key);
		},
		clear: async () => {
			entries.clear();
		},
	};
	return { store, entries };
};

/** A store whose writes land a moment after they are asked for, as IndexedDB's do. */
const slowStore = () => {
	const { store, entries } = memoryStore();
	const slow: CacheStore = {
		...store,
		set: async (key, value) => {
			await new Promise((resolve) => setTimeout(resolve, 10));
			await store.set(key, value);
		},
	};
	return { store: slow, entries };
};

const countingStore = () => {
	const { store, entries } = memoryStore();
	let writes = 0;
	const counting: CacheStore = {
		...store,
		set: async (key, value) => {
			writes += 1;
			await store.set(key, value);
		},
	};
	return { store: counting, entries, writes: () => writes };
};

const NOW = Date.parse("2026-09-14T00:00:00Z");
const KEY = ["project", "hashbrown"];

/** A cache that has seen `hashbrown`, saved for `reader`. */
const saved = async (reader = "reader-a", buster = "v1", savedAt = NOW) => {
	const { store, entries } = memoryStore();
	const client = createQueryClient(() => {});
	const { stop } = persistCache(client, store, reader, buster, {
		delay: 0,
		now: () => savedAt,
	});
	client.setQueryData(KEY, { name: "Hashbrown" });
	await vi.waitFor(() => expect(entries.has(reader)).toBe(true));
	stop();
	return { store, entries };
};

const restored = async (
	store: CacheStore,
	reader: string,
	buster = "v1",
	now = NOW,
) => {
	const client = createQueryClient(() => {});
	await restoreCache(client, store, reader, buster, now);
	return client.getQueryData(KEY);
};

describe("the persisted cache", () => {
	// Kills a cache that is never written, or never read back.
	test("a returning reader gets back what they saw", async () => {
		const { store } = await saved();
		expect(await restored(store, "reader-a")).toEqual({ name: "Hashbrown" });
	});

	// Kills a save that only hears answers that arrive after it starts, which
	// misses the shell's round when the API answers within the boot wait.
	test("what answered before saving began is saved too", async () => {
		const { store, entries } = memoryStore();
		const client = createQueryClient(() => {});
		client.setQueryData(KEY, { name: "Hashbrown" });
		const { stop } = persistCache(client, store, "reader-a", "v1", {
			delay: 0,
		});
		await vi.waitFor(() => expect(entries.has("reader-a")).toBe(true));
		stop();
		expect(await restored(store, "reader-a")).toEqual({ name: "Hashbrown" });
	});

	// Kills a cache shared between readers of one browser.
	test("another reader gets nothing", async () => {
		const { store } = await saved();
		expect(await restored(store, "reader-b")).toBeUndefined();
	});

	// Kills data of an older console read with newer code.
	test("a new version of the console starts empty and drops the old entry", async () => {
		const { store, entries } = await saved();
		expect(await restored(store, "reader-a", "v2")).toBeUndefined();
		expect(entries.has("reader-a")).toBe(false);
	});

	// Kills data kept forever.
	test("an entry older than the maximum age is not restored", async () => {
		const { store } = await saved();
		expect(
			await restored(store, "reader-a", "v1", NOW + MAX_AGE + 1),
		).toBeUndefined();
	});

	// Kills a save that hears only answers, or leaves a removal to the delayed
	// save a reload cancels, which leaves a deleted project at rest.
	test("a removed query leaves storage at once", async () => {
		const { store } = await saved();
		const client = createQueryClient(() => {});
		await restoreCache(client, store, "reader-a", "v1", NOW);
		const { stop } = persistCache(client, store, "reader-a", "v1", {
			delay: 60_000,
			now: () => NOW,
		});

		client.removeQueries({ queryKey: KEY });
		await vi.waitFor(async () =>
			expect(await restored(store, "reader-a")).toBeUndefined(),
		);
		stop();
	});

	// Kills a save after every answer, which rewrites the whole cache each time
	// a poll asks again and hears what it heard before.
	test("an answer that changes nothing is not saved again", async () => {
		const { store, writes } = countingStore();
		const client = createQueryClient(() => {});
		const { stop } = persistCache(client, store, "reader-a", "v1", {
			delay: 0,
			now: () => NOW,
		});
		let reports: { uuid: string }[] = [];
		const poll = () =>
			client.fetchQuery({
				queryKey: KEY,
				queryFn: async () => structuredClone(reports),
				staleTime: 0,
			});
		// What answered before saving began.
		await vi.waitFor(() => expect(writes()).toBe(1));
		await poll();
		await vi.waitFor(() => expect(writes()).toBe(2));

		await poll();
		await poll();
		await new Promise((resolve) => setTimeout(resolve, 20));
		expect(writes()).toBe(2);

		reports = [{ uuid: "first" }];
		await poll();
		await vi.waitFor(() => expect(writes()).toBe(3));
		stop();
	});

	// Kills a flush that leaves the change to a timer a navigation cancels.
	test("a flush writes every change at once", async () => {
		const { store, entries } = slowStore();
		const client = createQueryClient(() => {});
		const { flush, stop } = persistCache(client, store, "reader-a", "v1", {
			delay: 60_000,
			now: () => NOW,
		});
		client.setQueryData(KEY, { name: "Hashbrown" });

		await flush();
		expect(entries.get("reader-a")?.state.queries).toHaveLength(1);
		stop();
	});

	// Kills a flush that returns while a save already under way is still writing.
	test("a flush waits for a save already under way", async () => {
		const { store, entries } = slowStore();
		const client = createQueryClient(() => {});
		client.setQueryData(KEY, { name: "Hashbrown" });
		const { flush, stop } = persistCache(client, store, "reader-a", "v1", {
			delay: 0,
			now: () => NOW,
		});
		await new Promise((resolve) => setTimeout(resolve, 0));

		await flush();
		expect(entries.get("reader-a")?.state.queries).toHaveLength(1);
		stop();
	});

	// Kills an invalidation left to the delayed save a reload cancels, which
	// restores the data the reader's write replaced as fresh.
	test("an invalidation is saved at once", async () => {
		const { store, entries } = memoryStore();
		const client = createQueryClient(() => {});
		client.setQueryData(KEY, { name: "Hashbrown" });
		const { stop } = persistCache(client, store, "reader-a", "v1", {
			delay: 60_000,
			now: () => NOW,
		});

		await client.invalidateQueries({ queryKey: KEY, refetchType: "none" });
		await vi.waitFor(() => expect(entries.has("reader-a")).toBe(true));
		stop();
		const reloaded = createQueryClient(() => {});
		await restoreCache(reloaded, store, "reader-a", "v1", NOW);
		expect(reloaded.getQueryState(KEY)?.isInvalidated).toBe(true);
	});

	// Kills a write left to the delayed save a reload cancels, or to the one an
	// answer has pending, which restores the lists from before a revoke as
	// fresh, and a save for each list it moves.
	test("a write is saved at once, in one save", async () => {
		const { store, writes } = countingStore();
		const active = ["keys", "active"];
		const revoked = ["keys", "revoked"];
		const client = createQueryClient(() => {});
		client.setQueryData(active, ["ci"]);
		client.setQueryData(revoked, []);
		const { flush, stop } = persistCache(client, store, "reader-a", "v1", {
			delay: 60_000,
			now: () => NOW,
		});
		await flush();
		await client.query({ queryKey: KEY, queryFn: async () => ["answer"] });

		client.setQueryData(active, []);
		client.setQueryData(revoked, ["ci"]);
		await new Promise((resolve) => setTimeout(resolve, 20));
		stop();
		const reloaded = createQueryClient(() => {});
		await restoreCache(reloaded, store, "reader-a", "v1", NOW);
		expect(reloaded.getQueryData(revoked)).toEqual(["ci"]);
		expect(reloaded.getQueryData(active)).toEqual([]);
		expect(writes()).toBe(2);
	});

	// Kills a save for a write that changes nothing, which writes the whole
	// cache again each time an undo or a seed repeats what is saved.
	test("a write that changes nothing is not saved again", async () => {
		const { store, writes } = countingStore();
		const client = createQueryClient(() => {});
		client.setQueryData(KEY, { name: "Hashbrown" });
		const { flush, stop } = persistCache(client, store, "reader-a", "v1", {
			delay: 60_000,
			now: () => NOW,
		});
		await flush();

		client.setQueryData(KEY, { name: "Hashbrown" });
		await new Promise((resolve) => setTimeout(resolve, 20));
		stop();
		expect(writes()).toBe(1);
	});

	// Kills a save at once after every answer, which writes the whole cache
	// again each time a page reads.
	test("an answer waits for the delay", async () => {
		const { store, entries } = memoryStore();
		const client = createQueryClient(() => {});
		const { stop } = persistCache(client, store, "reader-a", "v1", {
			delay: 60_000,
			now: () => NOW,
		});

		await client.query({
			queryKey: KEY,
			queryFn: async () => ({ name: "Hashbrown" }),
		});
		await new Promise((resolve) => setTimeout(resolve, 20));
		stop();
		expect(entries.size).toBe(0);
	});

	// Kills another reader's data left at rest in the browser.
	test("a reader's boot removes every other reader's entry", async () => {
		const { store, entries } = await saved("reader-b");
		await store.set("reader-a", (await store.get("reader-b")) as Persisted);
		await purgeOtherReaders(store, "reader-a");
		expect([...entries.keys()]).toEqual(["reader-a"]);
	});

	// Kills a sign out that leaves data in memory or at rest.
	test("clearing empties memory and storage", async () => {
		const { store, entries } = await saved();
		const client = createQueryClient(() => {});
		client.setQueryData(KEY, { name: "Hashbrown" });
		await clearCache(client, store);
		expect(client.getQueryData(KEY)).toBeUndefined();
		expect(entries.size).toBe(0);
	});
});

describe("createQueryClient", () => {
	const attempts = async (error: ApiError) => {
		let calls = 0;
		const unauthorized = vi.fn();
		const client = createQueryClient(unauthorized);
		await client
			.query({
				queryKey: ["x"],
				queryFn: () => {
					calls += 1;
					throw error;
				},
				retryDelay: 0,
			})
			.catch(() => {});
		return { calls, unauthorized };
	};

	// Kills retries that hammer an API that refused, and none for a flaky one.
	test("retries a failure that may pass, and not a refusal", async () => {
		expect((await attempts(new ApiError(503, "server", ""))).calls).toBe(3);
		expect((await attempts(new ApiError(undefined, "network", ""))).calls).toBe(
			3,
		);
		expect((await attempts(new ApiError(404, "not_found", ""))).calls).toBe(1);
		expect((await attempts(new ApiError(401, "unauthorized", ""))).calls).toBe(
			1,
		);
	});

	// Kills a refused token that leaves the reader on a broken page.
	test("a refused token sends the reader to sign in, and nothing else does", async () => {
		expect(
			(await attempts(new ApiError(401, "unauthorized", ""))).unauthorized,
		).toHaveBeenCalledOnce();
		expect(
			(await attempts(new ApiError(404, "not_found", ""))).unauthorized,
		).not.toHaveBeenCalled();
	});
});
