import { Router } from "@solidjs/router";
import { type QueryClient, QueryClientProvider } from "@tanstack/solid-query";
import type { JsonAuthUser } from "../types/bencher";
import type { Api } from "./api";
import { FlushContext } from "./flush";
import Layout from "./Layout";
import type { BmfVersion } from "./memory";
import { NEXT_PROJECTS } from "./paths";
import { ROUTES } from "./routes";

const App = (props: {
	client: QueryClient;
	api: Api;
	reader: JsonAuthUser;
	versions: () => Record<string, BmfVersion>;
	flush: () => Promise<void>;
}) => (
	<QueryClientProvider client={props.client}>
		<FlushContext.Provider value={props.flush}>
			<Router
				base={NEXT_PROJECTS}
				root={(section) => (
					<Layout
						{...section}
						api={props.api}
						reader={props.reader}
						versions={props.versions}
					/>
				)}
			>
				{ROUTES}
			</Router>
		</FlushContext.Provider>
	</QueryClientProvider>
);

export default App;
