import "@bencherdev/ui/styles.css";
import "../styles/console.css";
import { MemoryRouter, Route, createMemoryHistory } from "@solidjs/router";
import { QueryClient, QueryClientProvider } from "@tanstack/solid-query";
import { type Component, Suspense } from "solid-js";
import { render } from "solid-js/web";
import type { JsonAuthUser } from "../../types/bencher";
import type { Api } from "../api";
import { AccountContext } from "./context";

const READER = {
	user: {
		uuid: "user-uuid",
		name: "Muriel Bagge",
		slug: "muriel-bagge",
		email: "muriel@nowhere.example",
		admin: false,
		locked: false,
	},
	token: "tok",
} as JsonAuthUser;

/** Render an account page at `/users/muriel-bagge/<segment>`, with `cached` answers already held. */
export const mountAccount = (
	Page: Component,
	segment: string,
	{ api, cached = [] }: { api: Api; cached?: [unknown[], unknown][] },
) => {
	const client = new QueryClient({
		defaultOptions: {
			queries: { retry: false, staleTime: Number.POSITIVE_INFINITY },
		},
	});
	for (const [key, data] of cached) {
		client.setQueryData(key, data);
	}
	const history = createMemoryHistory();
	history.set({
		value: `/users/${READER.user.slug}/${segment}`,
		replace: true,
	});
	const root = document.createElement("div");
	root.className = "console";
	document.body.append(root);
	const stop = render(
		() => (
			<QueryClientProvider client={client}>
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
							path={`/users/:user/${segment}`}
							component={() => (
								<Suspense fallback={<p>Suspended</p>}>
									<Page />
								</Suspense>
							)}
						/>
					</MemoryRouter>
				</AccountContext.Provider>
			</QueryClientProvider>
		),
		root,
	);
	return {
		client,
		dispose: () => {
			stop();
			root.remove();
		},
	};
};
