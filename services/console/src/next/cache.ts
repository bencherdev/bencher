import {
	type DehydratedState,
	QueryCache,
	QueryClient,
	dehydrate,
	hydrate,
} from "@tanstack/solid-query";
import { ApiError } from "./api";

/** Where a reader's cache rests between visits: one entry per reader. */
export interface CacheStore {
	get(reader: string): Promise<Persisted | undefined>;
	set(reader: string, value: Persisted): Promise<void>;
	keys(): Promise<string[]>;
	delete(reader: string): Promise<void>;
	clear(): Promise<void>;
}

export interface Persisted {
	/** The build that wrote it; any other build starts empty. */
	buster: string;
	savedAt: number;
	state: DehydratedState;
}

export const MAX_AGE = 7 * 24 * 60 * 60 * 1000;
const SAVE_DELAY = 1000;
const RETRIES = 2;

export const createQueryClient = (onUnauthorized: () => void) =>
	new QueryClient({
		queryCache: new QueryCache({
			onError: (error) => {
				if (error instanceof ApiError && error.kind === "unauthorized") {
					onUnauthorized();
				}
			},
		}),
		defaultOptions: {
			queries: {
				staleTime: 30_000,
				// Restored queries have no observers until a page asks for them.
				gcTime: MAX_AGE,
				retry: (failures, error) =>
					failures < RETRIES &&
					error instanceof ApiError &&
					(error.kind === "network" || error.kind === "server"),
			},
		},
	});

/** Put back what `reader` saw last time, unless another build or too long ago wrote it. */
export const restoreCache = async (
	client: QueryClient,
	store: CacheStore,
	reader: string,
	buster: string,
	now: number,
) => {
	const persisted = await store.get(reader);
	if (!persisted) {
		return;
	}
	if (persisted.buster !== buster || now - persisted.savedAt > MAX_AGE) {
		await store.delete(reader);
		return;
	}
	hydrate(client, persisted.state);
};

/** Save what the reader has seen, a moment after it changes. */
export const persistCache = (
	client: QueryClient,
	store: CacheStore,
	reader: string,
	buster: string,
	{ delay = SAVE_DELAY, now = Date.now } = {},
) => {
	let timer: ReturnType<typeof setTimeout> | undefined;
	let saving: Promise<void> = Promise.resolve();
	const save = () => {
		clearTimeout(timer);
		timer = undefined;
		saving = store
			.set(reader, { buster, savedAt: now(), state: dehydrate(client) })
			.catch(() => {});
		return saving;
	};
	const schedule = () => {
		timer ??= setTimeout(save, delay);
	};
	const unsubscribe = client.getQueryCache().subscribe((event) => {
		if (
			event.type === "removed" ||
			(event.type === "updated" && event.action.type === "success")
		) {
			schedule();
		}
	});
	// Queries that answered before this subscribed are saved too.
	schedule();
	return {
		/** Resolves once every change so far is written. */
		flush: () => (timer === undefined ? saving : save()),
		stop: () => {
			clearTimeout(timer);
			timer = undefined;
			unsubscribe();
		},
	};
};

export const purgeOtherReaders = async (store: CacheStore, reader: string) => {
	for (const key of await store.keys()) {
		if (key !== reader) {
			await store.delete(key);
		}
	}
};

export const clearCache = async (client: QueryClient, store: CacheStore) => {
	client.clear();
	await store.clear();
};
