import { createComputed, createRoot, createSignal } from "solid-js";
import { afterEach, expect, test } from "vitest";
import { useRequested } from "./requested";

let dispose: (() => void) | undefined;
afterEach(() => {
	dispose?.();
	dispose = undefined;
});

/** The hook over a query of letters, with `cached` the letters whose answers the cache holds, and every query it asked for. */
const requesting = (cached: readonly string[]) =>
	createRoot((done) => {
		dispose = done;
		const [query, setQuery] = createSignal("a");
		const { requested, pending } = useRequested(
			query,
			(letter) => letter,
			(letter) => cached.includes(letter),
		);
		const asked: string[] = [];
		createComputed(() => asked.push(requested()));
		return { setQuery, requested, pending, asked };
	});

// Kills a request for every edit in a run, rather than one once the reader pauses.
test("a run of edits asks once, for the last, after the reader pauses", async () => {
	const { setQuery, requested, pending, asked } = requesting([]);
	setQuery("b");
	setQuery("c");
	expect(pending()).toBe(true);
	await expect.poll(requested).toBe("c");
	expect(asked).toEqual(["a", "c"]);
	expect(pending()).toBe(false);
});

// Kills Back to a query the cache answers waiting out the pause.
test("a query whose answer the cache holds is asked for at once", () => {
	const { setQuery, requested, pending } = requesting(["b"]);
	setQuery("b");
	expect(requested()).toBe("b");
	expect(pending()).toBe(false);
});
