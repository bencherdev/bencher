/**
 * Written, and at once taken out, by a tab that proved this browser's session
 * again, so every other tab hears it: a dialog that asked moves on.
 */
const PROVED_KEY = "BENCHER_PROVED";

/** Tell every other tab that this browser's session was just proved again. */
export const signalProved = (
	storage: Pick<Storage, "setItem" | "removeItem">,
) => {
	try {
		storage.setItem(PROVED_KEY, "1");
		storage.removeItem(PROVED_KEY);
	} catch {
		// A browser that keeps nothing tells no other tab, which then asks again.
	}
};

/** Kept in a provider's window that a dialog opened to sign in again, for its return. */
const FRESH_WINDOW_KEY = "BENCHER_FRESH_WINDOW";

/** Mark a window, by its own tab storage, as opened to sign in again. */
export const keepFreshWindow = (storage: Pick<Storage, "setItem">) => {
	try {
		storage.setItem(FRESH_WINDOW_KEY, "1");
	} catch {
		// A browser that keeps nothing returns as a sign-in, which proves the session all the same.
	}
};

/** Whether this window was opened to sign in again, read once. */
export const takeFreshWindow = (
	storage: Pick<Storage, "getItem" | "removeItem">,
) => {
	try {
		const fresh = storage.getItem(FRESH_WINDOW_KEY) === "1";
		storage.removeItem(FRESH_WINDOW_KEY);
		return fresh;
	} catch {
		return false;
	}
};

/** Call `listener` each time another tab proves this browser's session again; returns its end. */
export const onProved = (
	listener: () => void,
	target: Pick<Window, "addEventListener" | "removeEventListener"> = window,
) => {
	const heard = (event: StorageEvent) => {
		if (event.key === PROVED_KEY && event.newValue !== null) {
			listener();
		}
	};
	target.addEventListener("storage", heard);
	return () => target.removeEventListener("storage", heard);
};
