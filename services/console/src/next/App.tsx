import { Router } from "@solidjs/router";
import { type QueryClient, QueryClientProvider } from "@tanstack/solid-query";
import { Show, createSignal } from "solid-js";
import type { JsonAuthUser, JsonUser } from "../types/bencher";
import AccountLayout from "./account/Layout";
import type { Api } from "./api";
import { FlushContext } from "./flush";
import Layout from "./Layout";
import type { BmfVersion } from "./memory";
import { NEXT_CONSOLE, accountUser } from "./paths";
import { rememberReader } from "./reader";
import { CONSOLE_ROUTES, PROJECTS_ROUTE } from "./routes";

const App = (props: {
	client: QueryClient;
	api: Api;
	reader: JsonAuthUser;
	versions: () => Record<string, BmfVersion>;
	flush: () => Promise<void>;
	signOut: () => void;
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
				<Router
					base={NEXT_CONSOLE}
					root={(section) => (
						<Show
							when={accountUser(section.location.pathname)}
							fallback={
								<Layout
									{...section}
									api={props.api}
									reader={reader()}
									versions={props.versions}
									host={props.host}
									route={PROJECTS_ROUTE}
								/>
							}
						>
							<AccountLayout
								{...section}
								api={props.api}
								reader={reader}
								keepUser={keepUser}
								signOut={props.signOut}
							/>
						</Show>
					)}
				>
					{CONSOLE_ROUTES}
				</Router>
			</FlushContext.Provider>
		</QueryClientProvider>
	);
};

export default App;
