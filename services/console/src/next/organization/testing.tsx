import "@bencherdev/ui/styles.css";
import "../styles/console.css";
import { MemoryRouter, Route, createMemoryHistory } from "@solidjs/router";
import { QueryClient, QueryClientProvider } from "@tanstack/solid-query";
import { type JSX, Suspense } from "solid-js";
import { render } from "solid-js/web";
import type {
	JsonAuthUser,
	JsonConsoleUser,
	JsonOrganization,
} from "../../types/bencher";
import { AccountContext } from "../account/context";
import type { Api } from "../api";
import { FlushContext } from "../flush";
import { consoleUserQuery } from "../queries";
import { type Allowed, allowedQuery, organizationQuery } from "./data";

export const ORGANIZATION: JsonOrganization = {
	uuid: "o1",
	name: "Pompeii LLC",
	slug: "pompeii-llc",
	created: "2026-09-01T00:00:00Z",
	modified: "2026-09-01T00:00:00Z",
};

const READER = {
	user: {
		uuid: "u",
		name: "Muriel Bagge",
		slug: "muriel-bagge",
		email: "muriel@nowhere.example",
		admin: false,
		locked: false,
	},
	token: "tok",
} as JsonAuthUser;

/** The reader's bootstrap: Pompeii LLC with Gizmo and Windmill, and Bagge Diner with Cathode. */
export const BOOTSTRAP: JsonConsoleUser = {
	uuid: "u",
	name: "Muriel Bagge",
	slug: "muriel-bagge",
	organizations: [
		{ uuid: "o1", name: "Pompeii LLC", slug: "pompeii-llc" },
		{ uuid: "o2", name: "Bagge Diner", slug: "bagge-diner" },
	],
	projects: [
		{
			uuid: "p1",
			name: "Gizmo",
			slug: "gizmo",
			organization: "o1",
			bmf_version: 1,
			active_alerts: 0,
		},
		{
			uuid: "p2",
			name: "Windmill",
			slug: "windmill",
			organization: "o1",
			bmf_version: 1,
			active_alerts: 0,
		},
		{
			uuid: "p3",
			name: "Cathode",
			slug: "cathode",
			organization: "o2",
			bmf_version: 1,
			active_alerts: 0,
		},
	],
};

/**
 * Render `part` at `path` under the console's base, as the organization's
 * pages are, with what the cache already holds: by default the reader's
 * bootstrap, Pompeii LLC, and a Leader's permissions.
 */
export const mount = (
	part: () => JSX.Element,
	{
		api,
		path = "/organizations/pompeii-llc/settings",
		organization = ORGANIZATION,
		allowed = { edit: true, delete: true },
		bootstrap = BOOTSTRAP,
		flush = async () => {},
		cached = [],
	}: {
		api: Api;
		path?: string;
		organization?: JsonOrganization | null;
		allowed?: Allowed | null;
		bootstrap?: JsonConsoleUser | null;
		flush?: () => Promise<void>;
		/** More answers the cache already holds, by query key. */
		cached?: [unknown[], unknown][];
	},
) => {
	const client = new QueryClient({
		defaultOptions: {
			queries: { retry: false, staleTime: Number.POSITIVE_INFINITY },
		},
	});
	const key = path.split("/")[2] ?? "";
	if (organization) {
		client.setQueryData(organizationQuery(api, key).queryKey, organization);
	}
	if (allowed) {
		client.setQueryData(allowedQuery(api, key).queryKey, allowed);
	}
	if (bootstrap) {
		client.setQueryData(consoleUserQuery(api, "u").queryKey, bootstrap);
	}
	for (const [queryKey, data] of cached) {
		client.setQueryData(queryKey, data);
	}
	const history = createMemoryHistory();
	history.set({ value: path, replace: true });
	const root = document.createElement("div");
	root.className = "console";
	document.body.append(root);
	const dispose = render(
		() => (
			<QueryClientProvider client={client}>
				<FlushContext.Provider value={flush}>
					<AccountContext.Provider
						value={{
							api,
							reader: () => READER,
							keepUser: () => {},
							signOut: () => {},
						}}
					>
						<MemoryRouter history={history}>
							<Route
								path="/organizations/:organization/*"
								component={() => (
									<Suspense fallback={<p>Suspended</p>}>{part()}</Suspense>
								)}
							/>
						</MemoryRouter>
					</AccountContext.Provider>
				</FlushContext.Provider>
			</QueryClientProvider>
		),
		root,
	);
	return {
		client,
		history,
		dispose: () => {
			dispose();
			root.remove();
		},
	};
};
