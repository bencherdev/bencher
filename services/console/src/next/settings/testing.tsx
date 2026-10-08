import "@bencherdev/ui/styles.css";
import "../styles/console.css";
import {
	MemoryRouter,
	Route,
	createMemoryHistory,
	useLocation,
} from "@solidjs/router";
import { QueryClient, QueryClientProvider } from "@tanstack/solid-query";
import { type JSX, Suspense } from "solid-js";
import { render } from "solid-js/web";
import {
	type JsonConsoleProject,
	type JsonProject,
	Visibility,
} from "../../types/bencher";
import type { Api, Method } from "../api";
import { ProjectContext } from "../project";

export const PROJECT: JsonProject = {
	uuid: "p",
	organization: "org-uuid",
	name: "Hashbrown",
	slug: "hashbrown",
	url: "https://github.com/pompeii-llc/hashbrown",
	visibility: Visibility.Public,
	bmf_version: 1,
	created: "2026-09-01T00:00:00Z",
	modified: "2026-09-01T00:00:00Z",
};

export const bootstrapOf = (
	permissions: Partial<JsonConsoleProject["permissions"]> = {},
): JsonConsoleProject => ({
	project: PROJECT,
	organization: { uuid: "org-uuid", name: "Pompeii LLC", slug: "pompeii-llc" },
	permissions: {
		view: true,
		create: true,
		edit: true,
		delete: true,
		manage: true,
		...permissions,
	},
	active_alerts: 1,
});

export interface Sent {
	method: Method;
	path: string;
	body: unknown;
}

/** An API that answers each change with `answer`, recording what it was sent, and each read with `read`, by default never. */
export const fakeApi = (
	answer: (sent: Sent) => Promise<unknown> = async () => undefined,
	read: (path: string) => Promise<never> = () => new Promise(() => {}),
) => {
	const sent: Sent[] = [];
	const api: Api = {
		get: read,
		send: async <T,>(method: Method, path: string, body?: unknown) => {
			const change = { method, path, body };
			sent.push(change);
			return { data: (await answer(change)) as T, headers: new Headers() };
		},
	};
	return { api, sent };
};

/** Render a Settings part for `hashbrown`, with the shell's answer already cached. */
export const mount = (
	part: () => JSX.Element,
	{
		api,
		bootstrap = bootstrapOf(),
		cached = [],
	}: {
		api: Api;
		bootstrap?: JsonConsoleProject;
		/** More answers the cache already holds, by query key. */
		cached?: [unknown[], unknown][];
	},
) => {
	const client = new QueryClient({
		defaultOptions: {
			queries: { retry: false, staleTime: Number.POSITIVE_INFINITY },
		},
	});
	client.setQueryData(["console", "project", "hashbrown"], bootstrap);
	for (const [key, data] of cached) {
		client.setQueryData(key, data);
	}
	const history = createMemoryHistory();
	history.set({ value: "/hashbrown/settings", replace: true });
	const root = document.createElement("div");
	root.className = "console";
	document.body.append(root);
	const dispose = render(
		() => (
			<QueryClientProvider client={client}>
				<MemoryRouter history={history}>
					<Route
						path="*"
						component={() => {
							// The slug follows the path, as the console's layout reads it.
							const location = useLocation();
							const slug = () => location.pathname.split("/")[1] ?? "";
							return (
								<ProjectContext.Provider value={{ api, slug }}>
									{/* As the console's layout holds every page. */}
									<Suspense fallback={<p>Suspended</p>}>{part()}</Suspense>
								</ProjectContext.Provider>
							);
						}}
					/>
				</MemoryRouter>
			</QueryClientProvider>
		),
		root,
	);
	return {
		client,
		history,
		root,
		dispose: () => {
			dispose();
			root.remove();
		},
	};
};

/** Watch `root` for the Suspense fallback; the returned call stops and says whether it showed. */
export const watchFallback = (root: HTMLElement) => {
	let shown = false;
	const observer = new MutationObserver((records) => {
		for (const record of records) {
			for (const node of record.addedNodes) {
				shown ||= node.textContent === "Suspended";
			}
		}
	});
	observer.observe(root, { childList: true, subtree: true });
	return () => {
		observer.disconnect();
		return shown;
	};
};
