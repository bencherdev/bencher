import { afterEach, expect, test } from "vitest";
import { READER_KEY } from "../next/reader";

const HINT = { uuid: "u", slug: "muriel-bagge", name: "Muriel" };

afterEach(() => localStorage.removeItem(READER_KEY));

// Kills a classic page that reads a reader from anything but the hint, or
// keeps one another tab signed out, or misses one another tab signed in.
test("the reader is the hint, and follows another tab's sign in and out", async () => {
	localStorage.setItem(READER_KEY, JSON.stringify(HINT));
	// Imported only now, since the module reads the hint as it loads.
	const { authUser } = await import("./auth");
	expect(authUser()).toEqual(HINT);

	localStorage.removeItem(READER_KEY);
	window.dispatchEvent(new StorageEvent("storage", { key: READER_KEY }));
	expect(authUser()).toBeUndefined();

	const other = { uuid: "v", slug: "courage", name: "Courage" };
	localStorage.setItem(READER_KEY, JSON.stringify(other));
	window.dispatchEvent(new StorageEvent("storage", { key: "BENCHER_THEME" }));
	expect(authUser()).toBeUndefined();
	window.dispatchEvent(new StorageEvent("storage", { key: null }));
	expect(authUser()).toEqual(other);
});
