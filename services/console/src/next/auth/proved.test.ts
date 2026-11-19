import { expect, test } from "vitest";
import {
	keepFreshWindow,
	onProved,
	signalProved,
	takeFreshWindow,
} from "./proved";

const memory = () => {
	const writes: [string, string | null][] = [];
	return {
		writes,
		setItem: (key: string, value: string) => {
			writes.push([key, value]);
		},
		removeItem: (key: string) => {
			writes.push([key, null]);
		},
	};
};

/** A window that hands `listener` each event a test sends it. */
const target = () => {
	const listeners = new Set<(event: StorageEvent) => void>();
	return {
		listeners,
		send: (event: Pick<StorageEvent, "key" | "newValue">) => {
			for (const listener of listeners) {
				listener(event as StorageEvent);
			}
		},
		addEventListener: (_type: string, listener: unknown) => {
			listeners.add(listener as (event: StorageEvent) => void);
		},
		removeEventListener: (_type: string, listener: unknown) => {
			listeners.delete(listener as (event: StorageEvent) => void);
		},
	};
};

// Kills a signal that other tabs never hear, one that fires only the first
// time, since a value written twice the same is no change, and one that
// leaves anything behind.
test("a proof is written and taken out at once", () => {
	const storage = memory();
	signalProved(storage);
	signalProved(storage);
	expect(storage.writes).toEqual([
		["BENCHER_PROVED", "1"],
		["BENCHER_PROVED", null],
		["BENCHER_PROVED", "1"],
		["BENCHER_PROVED", null],
	]);
});

// Kills a dialog that moves on for another key, or for the proof's own
// removal, and one that keeps listening once it ends.
test("only another tab's proof is heard, until the listener ends", () => {
	const window = target();
	let heard = 0;
	const end = onProved(() => {
		heard += 1;
	}, window);
	window.send({ key: "BENCHER_READER", newValue: "{}" });
	window.send({ key: "BENCHER_PROVED", newValue: null });
	expect(heard).toBe(0);
	window.send({ key: "BENCHER_PROVED", newValue: "1" });
	expect(heard).toBe(1);
	end();
	expect(window.listeners.size).toBe(0);
});

// Kills a browser that refuses storage taking the page down with it.
test("a browser that keeps nothing tells nothing", () => {
	expect(() =>
		signalProved({
			setItem: () => {
				throw new DOMException("refused", "SecurityError");
			},
			removeItem: () => {},
		}),
	).not.toThrow();
});

// Kills a provider's window that forgets it was opened to sign in again, one
// that reads as opened again on a reload, and a sign-in's window that reads
// as one.
test("a window opened to sign in again knows it, once", () => {
	const kept = new Map<string, string>();
	const storage = {
		setItem: (key: string, value: string) => kept.set(key, value),
		getItem: (key: string) => kept.get(key) ?? null,
		removeItem: (key: string) => kept.delete(key),
	};
	expect(takeFreshWindow(storage)).toBe(false);
	keepFreshWindow(storage);
	expect(takeFreshWindow(storage)).toBe(true);
	expect(takeFreshWindow(storage)).toBe(false);
	expect(kept.size).toBe(0);
});

// Kills a browser that refuses storage taking the provider's window down.
test("a window that keeps nothing returns as a sign-in", () => {
	const refuse = () => {
		throw new DOMException("refused", "SecurityError");
	};
	expect(() => keepFreshWindow({ setItem: refuse })).not.toThrow();
	expect(takeFreshWindow({ getItem: refuse, removeItem: refuse })).toBe(false);
});
