import type { QueryClient } from "@tanstack/solid-query";
import { render } from "solid-js/web";
import type { JsonAuthUser } from "../types/bencher";
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
import { forgetShell } from "./memory";
import { parseNextPath, tabOf } from "./paths";
import { consoleProjectQuery, fetchedVersions } from "./queries";
import { USER_KEY, isCurrent, readReader, signInHref } from "./reader";
import { NOT_FOUND, PAGES } from "./routes";

/** The longest the first render waits on the cache and the page's code. */
const BOOT_WAIT = 100;

const boot = async (mount: HTMLElement) => {
	const store = idbStore();
	const reader = readReader(localStorage);
	if (!reader || !isCurrent(reader, Date.now())) {
		await signOut(store);
		return;
	}

	const client = createQueryClient(() => signOut(store, client));
	const versions = fetchedVersions(client);
	const api = createApi({
		url: mount.dataset.apiUrl ?? "",
		token: reader.token,
		expiration: Date.parse(reader.expiration),
	});
	const place = parseNextPath(location.pathname);
	// The shell's one request starts before anything else, so the cache restore
	// never delays it.
	if (place) {
		client.query(consoleProjectQuery(api, place.slug)).catch(() => {});
	}

	// Each build has its own chunk name, so a new console never reads an old cache.
	const buster = new URL(import.meta.url).pathname;
	const tab = place && tabOf(place.rest);
	const page = tab ? PAGES[tab] : NOT_FOUND;
	await within(
		BOOT_WAIT,
		Promise.all([
			restoreCache(client, store, reader.user.uuid, buster, Date.now()),
			purgeOtherReaders(store, reader.user.uuid),
			page.preload(),
		]),
	);
	const persister = persistCache(client, store, reader.user.uuid, buster);
	stopSaving = persister.stop;
	watchReader(reader, store, client);

	handOff(mount, () =>
		render(
			() => (
				<App
					client={client}
					api={api}
					reader={reader}
					versions={versions}
					flush={persister.flush}
				/>
			),
			mount,
		),
	);
};

// Leaving stops the saves first, so none writes back what was just cleared.
let stopSaving = () => {};
let signingOut = false;

/** Forget the reader and everything they saw, then sign in and come back here. */
const signOut = async (store: CacheStore, client?: QueryClient) => {
	if (signingOut) {
		return;
	}
	signingOut = true;
	stopSaving();
	localStorage.removeItem(USER_KEY);
	forgetShell(localStorage);
	await within(BOOT_WAIT, client ? clearCache(client, store) : store.clear());
	location.replace(
		signInHref(`${location.pathname}${location.search}${location.hash}`),
	);
};

/** Signing out, or in as someone else, in another tab ends this session too. */
const watchReader = (
	reader: JsonAuthUser,
	store: CacheStore,
	client: QueryClient,
) => {
	window.addEventListener("storage", (event) => {
		if (event.key !== USER_KEY && event.key !== null) {
			return;
		}
		const current = readReader(localStorage);
		if (
			current?.user.uuid === reader.user.uuid &&
			current.token === reader.token
		) {
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
