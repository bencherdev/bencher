import type { QueryClient } from "@tanstack/solid-query";
import { render } from "solid-js/web";
import App from "./App";
import { createApi } from "./api";
import {
	type CacheStore,
	clearCache,
	createQueryClient,
	persistCache,
	purgeOtherReaders,
	restoreCache,
} from "./cache";
import { handOff } from "./handoff";
import { idbStore } from "./idb";
import { forgetReader } from "./memory";
import { type PageName, pageName, parseConsolePath } from "./paths";
import { consoleProjectQuery } from "./queries";
import { READER_KEY, type Reader, readReader, signInHref } from "./reader";
import { PAGES } from "./routes";

/** The longest the first render waits on the cache and the page's code. */
const BOOT_WAIT = 100;

const boot = async (mount: HTMLElement) => {
	const store = idbStore();
	const reader = readReader(localStorage);
	if (!reader) {
		await signOut(store);
		return;
	}

	const client = createQueryClient();
	const host = mount.dataset.apiUrl ?? "";
	const api = createApi({ onUnauthorized: () => signOut(store, client) });
	const place = parseConsolePath(location.pathname);
	// The shell's one request starts before anything else, so the cache restore
	// never delays it.
	if (place) {
		client.query(consoleProjectQuery(api, place.slug)).catch(() => {});
	}

	// Each build has its own chunk name, so a new console never reads an old cache.
	const buster = new URL(import.meta.url).pathname;
	// An account or organization document names its page, so the entry carries neither table.
	const page =
		PAGES[
			place
				? pageName(place.rest)
				: ((mount.dataset.page as PageName | undefined) ?? "NotFound")
		];
	await within(
		BOOT_WAIT,
		Promise.all([
			restoreCache(client, store, reader.uuid, buster, Date.now()),
			purgeOtherReaders(store, reader.uuid),
			page.preload(),
		]),
	);
	const persister = persistCache(client, store, reader.uuid, buster);
	stopSaving = persister.stop;
	watchReader(reader, store, client);

	handOff(mount, () =>
		render(
			() => (
				<App
					client={client}
					api={api}
					reader={reader}
					flush={persister.flush}
					signOut={() => signOut(store, client, true)}
					host={host}
				/>
			),
			mount,
		),
	);
};

// Leaving stops the saves first, so none writes back what was just cleared.
let stopSaving = () => {};
let signingOut = false;

/**
 * Forget the reader and everything they saw, then sign in again, coming back
 * here; or, when the reader signs out, end the session first and open the door.
 */
const signOut = async (
	store: CacheStore,
	client?: QueryClient,
	end = false,
) => {
	if (signingOut) {
		return;
	}
	signingOut = true;
	stopSaving();
	if (end) {
		// Loaded only to sign out, which no page needs to paint; the reader is
		// forgotten here even when it does not load.
		await import("./session")
			.then(({ endSession }) => endSession())
			.catch(() => {});
	}
	forgetReader(localStorage);
	await within(BOOT_WAIT, client ? clearCache(client, store) : store.clear());
	location.replace(
		end
			? "/auth/login"
			: signInHref(`${location.pathname}${location.search}${location.hash}`),
	);
};

/** Signing out, or in as someone else, in another tab ends this session too. */
const watchReader = (
	reader: Reader,
	store: CacheStore,
	client: QueryClient,
) => {
	window.addEventListener("storage", (event) => {
		if (event.key !== READER_KEY && event.key !== null) {
			return;
		}
		const current = readReader(localStorage);
		if (current?.uuid === reader.uuid) {
			return;
		}
		if (current) {
			stopSaving();
			within(BOOT_WAIT, clearCache(client, store)).then(() =>
				location.reload(),
			);
		} else {
			signOut(store, client);
		}
	});
};

/** Wait for `work`, but no longer than `ms`; a failure only ends the wait. */
const within = (ms: number, work: Promise<unknown>) =>
	Promise.race([
		work.catch(() => undefined),
		new Promise((resolve) => setTimeout(resolve, ms)),
	]);

const mount = document.getElementById("console");
if (mount) {
	boot(mount);
}
