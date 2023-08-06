import {
	createContext,
	useContext,
	createResource,
	Context,
	Resource,
	Accessor,
} from "solid-js";
import bencher_valid_init, { InitOutput } from "bencher_valid";
import { authUser } from "../../util/auth";
import { organizationSlug, projectSlug } from "../../util/url";
import type { JsonAuthUser } from "../../types/bencher";

export interface Console {
	user: Accessor<JsonAuthUser>;
	organizationSlug: Accessor<null | string>;
	projectSlug: Accessor<null | string>;
}

const ConsoleContext: Context<undefined | Resource<Console>> = createContext();

export function ConsoleProvider(props: { children: any }) {
	const [bencher_valid] = createResource(
		async () => await bencher_valid_init(),
	);

	const consoleResource = async (_wasm: undefined | InitOutput) => {
		return {
			user: authUser,
			organizationSlug: organizationSlug,
			projectSlug: projectSlug,
		};
	};
	const [console] = createResource(bencher_valid, consoleResource);

	return (
		<ConsoleContext.Provider value={console}>
			{props.children}
		</ConsoleContext.Provider>
	);
}

export function useConsole() {
	return useContext(ConsoleContext);
}
