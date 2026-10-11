import { afterEach, expect, test, vi } from "vitest";
import { deleteCache } from "./idb";

afterEach(() => {
	vi.unstubAllGlobals();
});

// Kills a sign out that a browser refusing IndexedDB stops halfway.
test("a browser that refuses IndexedDB still signs out", () => {
	vi.stubGlobal("indexedDB", {
		deleteDatabase: () => {
			throw new DOMException("refused", "SecurityError");
		},
	});
	expect(() => deleteCache()).not.toThrow();
});
