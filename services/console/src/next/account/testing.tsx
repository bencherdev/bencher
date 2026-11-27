import "@bencherdev/ui/styles.css";
import "../styles/console.css";
import { MemoryRouter, Route, createMemoryHistory } from "@solidjs/router";
import { QueryClient, QueryClientProvider } from "@tanstack/solid-query";
import { type Component, Suspense, createSignal } from "solid-js";
import { render } from "solid-js/web";
import type { Api } from "../api";
import { type Reader, readerOf } from "../reader";
import { AccountContext } from "./context";

export const READER: Reader = {
	uuid: "user-uuid",
	name: "Muriel Bagge",
	slug: "muriel-bagge",
};

/** Render an account page at `/users/muriel-bagge/<segment>`, with `cached` answers already held. */
export const mountAccount = (
	Page: Component,
	segment: string,
	{
		api,
		cached = [],
		reader = READER,
	}: { api: Api; cached?: [unknown[], unknown][]; reader?: Reader },
) => {
	const client = new QueryClient({
		defaultOptions: {
			queries: { retry: false, staleTime: Number.POSITIVE_INFINITY },
		},
	});
	for (const [key, data] of cached) {
		client.setQueryData(key, data);
	}
	const [kept, keep] = createSignal(reader);
	const history = createMemoryHistory();
	history.set({
		value: `/users/${READER.slug}/${segment}`,
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
						reader: kept,
						keepUser: (user) => keep(readerOf(user)),
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
		/** The reader as the page last kept them. */
		reader: kept,
		dispose: () => {
			stop();
			root.remove();
		},
	};
};
