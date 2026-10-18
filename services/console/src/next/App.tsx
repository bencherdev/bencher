import { Router } from "@solidjs/router";
import { type QueryClient, QueryClientProvider } from "@tanstack/solid-query";
import { Match, Suspense, Switch, createSignal } from "solid-js";
import type { JsonAuthUser, JsonUser } from "../types/bencher";
import { AccountContext } from "./account/context";
import AccountLayout from "./account/Layout";
import type { Api } from "./api";
import { FlushContext } from "./flush";
import Layout from "./Layout";
import type { BmfVersion } from "./memory";
import { NEXT_CONSOLE, accountUser, organizationOf } from "./paths";
import { rememberReader } from "./reader";
import { CONSOLE_ROUTES, OrganizationLayout, PROJECTS_ROUTE } from "./routes";

const App = (props: {
	client: QueryClient;
	api: Api;
	reader: JsonAuthUser;
	versions: () => Record<string, BmfVersion>;
	flush: () => Promise<void>;
	/** Ends the session; a sign in comes back here unless `back` is false. */
	signOut: (back?: boolean) => void;
	/** The API's URL. */
	host: string;
}) => {
	const [reader, setReader] = createSignal(props.reader);
	const keepUser = (user: JsonUser) => {
		const next = { ...reader(), user };
		setReader(next);
		rememberReader(localStorage, next);
	};
	return (
		<QueryClientProvider client={props.client}>
			<FlushContext.Provider value={props.flush}>
				<AccountContext.Provider
					value={{
						api: props.api,
						reader,
						keepUser,
						signOut: props.signOut,
					}}
				>
					<Router
						base={NEXT_CONSOLE}
						root={(section) => (
							<Switch
								fallback={
									<Layout
										{...section}
										api={props.api}
										reader={reader()}
										versions={props.versions}
										host={props.host}
										route={PROJECTS_ROUTE}
										signOut={props.signOut}
									/>
								}
							>
								<Match when={accountUser(section.location.pathname)}>
									<AccountLayout>{section.children}</AccountLayout>
								</Match>
								<Match when={organizationOf(section.location.pathname)}>
									<Suspense>
										<OrganizationLayout>{section.children}</OrganizationLayout>
									</Suspense>
								</Match>
							</Switch>
						)}
					>
						{CONSOLE_ROUTES}
					</Router>
				</AccountContext.Provider>
			</FlushContext.Provider>
		</QueryClientProvider>
	);
};

export default App;
