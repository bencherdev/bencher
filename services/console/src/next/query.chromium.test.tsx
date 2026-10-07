import {
	QueryClient,
	QueryClientProvider,
	focusManager,
} from "@tanstack/solid-query";
import { Show, Suspense, createSignal } from "solid-js";
import { render } from "solid-js/web";
import { afterEach, describe, expect, test } from "vitest";
import { page } from "vitest/browser";
import { useQueryResult } from "./query";

let dispose: (() => void) | undefined;

/** A query whose answers count up, and fail while `failing` is set. */
const counted = () => {
	const calls: string[] = [];
	const state = {
		failing: false,
		hold: undefined as Promise<void> | undefined,
	};
	const queryFn = async (key: string) => {
		calls.push(key);
		await state.hold;
		if (state.failing) {
			throw new Error(`${key} refused`);
		}
		return `${key} ${calls.length}`;
	};
	return { calls, state, queryFn };
};

const mount = (
	client: QueryClient,
	options: {
		key: () => string;
		enabled?: () => boolean;
		shown?: () => boolean;
		queryFn: (key: string) => Promise<string>;
	},
) => {
	const Reader = () => {
		const result = useQueryResult<string>(() => ({
			queryKey: ["probe", options.key()],
			queryFn: () => options.queryFn(options.key()),
			enabled: options.enabled?.() ?? true,
		}));
		return (
			<p>
				<output>{result().data ?? "nothing"}</output>
				<Show when={result().error}>
					{(error) => <span role="alert">{error().message}</span>}
				</Show>
			</p>
		);
	};
	const root = document.createElement("div");
	document.body.append(root);
	dispose = render(
		() => (
			<QueryClientProvider client={client}>
				<Suspense fallback={<span>Loading</span>}>
					<Show when={options.shown?.() ?? true}>
						<Reader />
					</Show>
				</Suspense>
			</QueryClientProvider>
		),
		root,
	);
	return root;
};

const shown = (root: Element) => root.querySelector("output")?.textContent;
const observers = (client: QueryClient, key: string) =>
	client
		.getQueryCache()
		.find({ queryKey: ["probe", key] })
		?.getObserversCount() ?? 0;

const newClient = () =>
	new QueryClient({ defaultOptions: { queries: { retry: false } } });

afterEach(() => {
	dispose?.();
	document.body.replaceChildren();
	focusManager.setFocused(undefined);
});

describe("useQueryResult", () => {
	// Kills an observer left subscribed after its page is gone, which keeps
	// refetching for nobody.
	test("one observer while mounted and none after", async () => {
		const client = newClient();
		const { queryFn } = counted();
		const [visible, setVisible] = createSignal(true);
		const root = mount(client, { key: () => "a", shown: visible, queryFn });
		await expect.poll(() => shown(root)).toBe("a 1");
		expect(observers(client, "a")).toBe(1);

		setVisible(false);
		expect(observers(client, "a")).toBe(0);
	});

	// Kills an observer that keeps reading the options it started with.
	test("follows its options when they change", async () => {
		const client = newClient();
		const { queryFn } = counted();
		const [key, setKey] = createSignal("a");
		const root = mount(client, { key, queryFn });
		await expect.poll(() => shown(root)).toBe("a 1");

		setKey("b");
		await expect.poll(() => shown(root)).toBe("b 2");
		expect(observers(client, "a")).toBe(0);
		expect(observers(client, "b")).toBe(1);
	});

	// Kills a result read once and never updated by the cache's refetches.
	test("shows what an invalidation or a window focus refetches", async () => {
		const client = newClient();
		const { queryFn } = counted();
		const root = mount(client, { key: () => "a", queryFn });
		await expect.poll(() => shown(root)).toBe("a 1");

		await client.invalidateQueries({ queryKey: ["probe", "a"] });
		await expect.poll(() => shown(root)).toBe("a 2");

		focusManager.setFocused(false);
		focusManager.setFocused(true);
		await expect.poll(() => shown(root)).toBe("a 3");
	});

	// Kills a result that waits for a fetch to show the console's own writes.
	test("shows a write to the cache at once", async () => {
		const client = newClient();
		const { queryFn } = counted();
		const root = mount(client, { key: () => "a", queryFn });
		await expect.poll(() => shown(root)).toBe("a 1");

		client.setQueryData(["probe", "a"], "written");
		expect(shown(root)).toBe("written");
	});

	// Kills a failed refetch that blanks what the reader already sees.
	test("a failed refetch keeps the last data beside the error", async () => {
		const client = newClient();
		const { queryFn, state } = counted();
		const root = mount(client, { key: () => "a", queryFn });
		await expect.poll(() => shown(root)).toBe("a 1");

		state.failing = true;
		await client.refetchQueries({ queryKey: ["probe", "a"] });
		await expect
			.element(page.getByRole("alert"))
			.toHaveTextContent("a refused");
		expect(shown(root)).toBe("a 1");
	});

	// Kills a disabled query that fetches anyway.
	test("a disabled query stays idle", async () => {
		const client = newClient();
		const { queryFn, calls } = counted();
		const root = mount(client, {
			key: () => "a",
			enabled: () => false,
			queryFn,
		});
		await new Promise((resolve) => setTimeout(resolve, 50));
		expect(calls).toEqual([]);
		expect(shown(root)).toBe("nothing");
	});

	// Kills a read through a resource, which suspends the nearest boundary and
	// swaps the page for its fallback when a query refetches what was seen.
	test("a refetch of data already seen never shows the Suspense fallback", async () => {
		const client = newClient();
		client.setQueryData(["probe", "a"], "seen");
		const { queryFn, state } = counted();
		let release = () => {};
		state.hold = new Promise((resolve) => {
			release = resolve;
		});
		const fallbacks: Node[] = [];
		const watcher = new MutationObserver((records) => {
			for (const record of records) {
				for (const node of record.addedNodes) {
					if (node.textContent === "Loading") {
						fallbacks.push(node);
					}
				}
			}
		});
		watcher.observe(document.body, { childList: true, subtree: true });

		const root = mount(client, { key: () => "a", queryFn });
		expect(shown(root)).toBe("seen");
		const output = root.querySelector("output");
		await new Promise(requestAnimationFrame);
		release();
		await expect.poll(() => shown(root)).toBe("a 1");
		await client.invalidateQueries({ queryKey: ["probe", "a"] });
		await expect.poll(() => shown(root)).toBe("a 2");
		watcher.disconnect();

		expect(fallbacks).toEqual([]);
		expect(root.querySelector("output")).toBe(output);
	});
});
