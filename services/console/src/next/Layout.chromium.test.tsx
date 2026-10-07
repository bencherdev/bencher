import "@bencherdev/ui/styles.css";
import "./styles/console.css";
import { MemoryRouter, createMemoryHistory } from "@solidjs/router";
import { QueryClient, QueryClientProvider } from "@tanstack/solid-query";
import { render } from "solid-js/web";
import { afterEach, describe, expect, test } from "vitest";
import { page } from "vitest/browser";
import type { JsonAuthUser, JsonConsoleProject } from "../types/bencher";
import type { Api } from "./api";
import Layout from "./Layout";
import { NEXT_PROJECTS } from "./paths";
import { consoleProjectQuery } from "./queries";
import { PAGES, ROUTES } from "./routes";

const reader = {
	user: { uuid: "reader-uuid", slug: "muriel-bagge" },
} as JsonAuthUser;

const bootstrap = (alerts: number) =>
	({
		project: { uuid: "project-uuid", name: "Hashbrown", slug: "hashbrown" },
		organization: { uuid: "org-uuid", name: "Pompeii LLC", slug: "pompeii" },
		permissions: {
			view: true,
			create: true,
			edit: true,
			delete: true,
			manage: true,
		},
		active_alerts: alerts,
	}) as JsonConsoleProject;

/** An API whose every answer waits until the test gives it. */
const heldApi = () => {
	const answers: ((data: unknown) => void)[] = [];
	const api: Api = {
		get: <T,>() =>
			new Promise<{ data: T; headers: Headers }>((resolve) => {
				answers.push((data) =>
					resolve({ data: data as T, headers: new Headers() }),
				);
			}),
	};
	return { api, answers };
};

let dispose: (() => void) | undefined;

const mount = (client: QueryClient, api: Api) => {
	const history = createMemoryHistory();
	history.set({ value: `${NEXT_PROJECTS}/hashbrown/reports` });
	const root = document.createElement("div");
	root.className = "console";
	document.body.append(root);
	dispose = render(
		() => (
			<QueryClientProvider client={client}>
				<MemoryRouter
					history={history}
					base={NEXT_PROJECTS}
					root={(section) => (
						<Layout
							{...section}
							api={api}
							reader={reader}
							versions={() => ({})}
						/>
					)}
				>
					{ROUTES}
				</MemoryRouter>
			</QueryClientProvider>
		),
		root,
	);
	return root;
};

const heading = () => page.getByRole("heading", { level: 1, name: "Reports" });
const busy = (root: Element) => root.querySelector('[aria-busy="true"]');

afterEach(() => {
	dispose?.();
	document.body.replaceChildren();
	localStorage.clear();
});

describe("the layout", () => {
	// Kills a page that paints before the shell's answer says what the reader may do.
	test("a project never seen shows the skeleton until the shell answers", async () => {
		await PAGES.reports.preload();
		const { api, answers } = heldApi();
		const root = mount(new QueryClient(), api);

		await expect.poll(() => answers.length).toBe(1);
		expect(busy(root)).not.toBeNull();
		expect(heading().query()).toBeNull();

		answers[0]?.(bootstrap(1));
		await expect.element(heading()).toBeVisible();
		expect(busy(root)).toBeNull();
	});

	// Kills a read of the shell's query that suspends the page: every refetch
	// then swaps the page for its skeleton, dropping focus and an open dialog's modality.
	test("a refetch of the shell's query keeps the page in place", async () => {
		const { api, answers } = heldApi();
		const client = new QueryClient({
			defaultOptions: { queries: { staleTime: Number.POSITIVE_INFINITY } },
		});
		client.setQueryData(
			consoleProjectQuery(api, "hashbrown").queryKey,
			bootstrap(1),
		);
		const root = mount(client, api);
		await expect.element(heading()).toBeVisible();
		const shown = heading().element();

		const skeletons: Node[] = [];
		const watcher = new MutationObserver((records) => {
			for (const record of records) {
				for (const node of record.addedNodes) {
					if (
						node instanceof Element &&
						(node.matches('[aria-busy="true"]') || busy(node))
					) {
						skeletons.push(node);
					}
				}
			}
		});
		watcher.observe(root, { childList: true, subtree: true });

		client.invalidateQueries();
		await expect.poll(() => answers.length).toBe(1);
		await new Promise(requestAnimationFrame);
		answers[0]?.(bootstrap(2));
		await expect
			.element(page.getByRole("link", { name: "Alerts 2 active" }))
			.toBeVisible();
		watcher.disconnect();

		expect(skeletons).toEqual([]);
		expect(heading().element()).toBe(shown);
	});
});
