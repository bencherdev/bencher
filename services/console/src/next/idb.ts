import type { CacheStore, Persisted } from "./cache";

/** The database the classic console deletes when the reader signs out. */
export const CACHE_DB = "bencher-console";
const STORE = "cache";

/** Signing out deletes what the reader saw. */
export const deleteCache = () => {
	try {
		indexedDB.deleteDatabase(CACHE_DB);
	} catch {
		// A browser that refuses IndexedDB has nothing to delete.
	}
};

/** One IndexedDB object store, one entry per reader. */
export const idbStore = (): CacheStore => {
	let db: Promise<IDBDatabase> | undefined;
	const open = () => {
		db ??= new Promise((resolve, reject) => {
			const request = indexedDB.open(CACHE_DB, 1);
			request.onupgradeneeded = () => request.result.createObjectStore(STORE);
			request.onsuccess = () => {
				const connection = request.result;
				// Another tab signing out deletes the database; this tab lets go and
				// never writes the cache back.
				connection.onversionchange = () => {
					connection.close();
					db = Promise.reject(new Error("The cache was deleted."));
					db.catch(() => {});
				};
				resolve(connection);
			};
			request.onerror = () => reject(request.error);
		});
		return db;
	};
	const run = async <T>(
		mode: IDBTransactionMode,
		act: (store: IDBObjectStore) => IDBRequest<T>,
	) => {
		const transaction = (await open()).transaction(STORE, mode);
		const request = act(transaction.objectStore(STORE));
		return new Promise<T>((resolve, reject) => {
			transaction.oncomplete = () => resolve(request.result);
			transaction.onerror = () => reject(transaction.error);
			transaction.onabort = () => reject(transaction.error);
		});
	};
	return {
		get: (reader) =>
			run("readonly", (store) => store.get(reader) as IDBRequest<Persisted>),
		set: async (reader, value) => {
			await run("readwrite", (store) => store.put(value, reader));
		},
		keys: async () =>
			(await run("readonly", (store) => store.getAllKeys())).map(String),
		delete: async (reader) => {
			await run("readwrite", (store) => store.delete(reader));
		},
		clear: async () => {
			await run("readwrite", (store) => store.clear());
		},
	};
};
