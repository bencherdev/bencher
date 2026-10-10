import { expect, test } from "vitest";
import type { Persisted } from "./cache";
import { CACHE_DB, idbStore } from "./idb";

/** How another tab's sign out ends: deleted, or blocked by a connection this tab holds open. */
const deleteFromAnotherTab = () =>
	new Promise<string>((resolve) => {
		const request = indexedDB.deleteDatabase(CACHE_DB);
		request.onsuccess = () => resolve("deleted");
		request.onerror = () => resolve("failed");
		request.onblocked = () => resolve("blocked");
	});

// Kills an open tab that holds the cache past another tab's sign out, and one
// that writes the signed out reader's cache back.
test("another tab's sign out deletes the cache this tab has open", async () => {
	const store = idbStore();
	await store.set("reader-a", {} as Persisted);

	expect(await deleteFromAnotherTab()).toBe("deleted");
	await expect(store.set("reader-a", {} as Persisted)).rejects.toBeDefined();
	expect((await indexedDB.databases()).map(({ name }) => name)).not.toContain(
		CACHE_DB,
	);
});
