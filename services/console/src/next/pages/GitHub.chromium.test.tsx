import "@bencherdev/ui/styles.css";
import "../styles/console.css";
import {
	MemoryRouter,
	Route,
	createMemoryHistory,
	useParams,
} from "@solidjs/router";
import { QueryClient, QueryClientProvider } from "@tanstack/solid-query";
import { Suspense } from "solid-js";
import { render } from "solid-js/web";
import { afterEach, expect, test } from "vitest";
import { page } from "vitest/browser";
import {
	GitHubInstallationPermission,
	type JsonAuthUser,
	type JsonGitHubRepositories,
} from "../../types/bencher";
import { AccountContext } from "../account/context";
import { ApiError } from "../api";
import { RETURN_KEY } from "../astro/githubReturn";
import { type CacheStore, persistCache } from "../cache";
import { ProjectContext } from "../project";
import Connections, { githubQuery, hold } from "../settings/Connections";
import { type Sent, bootstrapOf, fakeApi } from "../settings/testing";
import GitHub from "./GitHub";

let dispose: (() => void) | undefined;

afterEach(() => {
	dispose?.();
	dispose = undefined;
	hold(undefined);
	Reflect.deleteProperty(window, RETURN_KEY);
	localStorage.clear();
	sessionStorage.clear();
});

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

const GRANT = "eyJhbGciOiJIUzI1NiJ9.eyJyZXBvIjoxfQ.Z3JhbnQ";
const CODE = "github-code";
/** A link state as the API signs one: its claims name the project's UUID. */
const STATE = `eyJhbGciOiJIUzI1NiJ9.${btoa(JSON.stringify({ sub: "u", project: "p" })).replace(/=+$/, "")}.c2ln`;

const EXCHANGED: JsonGitHubRepositories = {
	project: "hashbrown",
	repositories: [
		{
			full_name: "pompeii-llc/hashbrown",
			installation: 1,
			installation_url: "https://github.example/organizations/pompeii-llc",
			permission: GitHubInstallationPermission.Granted,
			grant: GRANT,
		},
	],
};

/**
 * The return page under the console's base, with what its first script took
 * from GitHub's URL, beside the project pages it moves to, which draw
 * Connections; with `persisted`, the cache saves what it holds as the
 * console's does, to a store that keeps it as text.
 */
const open = (
	kept: { code?: string; state?: string },
	{
		answer,
		read = async () => answered({}),
		persisted = false,
	}: {
		answer: (sent: Sent) => Promise<unknown>;
		read?: (path: string) => Promise<never>;
		persisted?: boolean;
	},
) => {
	const { api, sent } = fakeApi(answer, read);
	const client = new QueryClient({
		defaultOptions: {
			queries: { retry: false, staleTime: Number.POSITIVE_INFINITY },
		},
	});
	client.setQueryData(["console", "project", "hashbrown"], bootstrapOf());
	const saved: string[] = [];
	const persister = persisted
		? persistCache(client, recording(saved), READER.user.uuid, "build", {
				delay: 0,
			})
		: undefined;
	Reflect.set(window, RETURN_KEY, kept);
	const history = createMemoryHistory();
	history.set({ value: "/next/console/github", replace: true });
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
					<MemoryRouter history={history} base="/next/console">
						<Route
							path="/github"
							component={() => (
								<Suspense fallback={<p>Suspended</p>}>
									<GitHub />
								</Suspense>
							)}
						/>
						<Route
							path="/projects/:project/connections"
							component={() => {
								const params = useParams();
								return (
									<ProjectContext.Provider
										value={{
											api,
											slug: () => params.project ?? "",
											route: "/projects",
										}}
									>
										<Suspense fallback={<p>Suspended</p>}>
											<Connections />
										</Suspense>
									</ProjectContext.Provider>
								);
							}}
						/>
					</MemoryRouter>
				</AccountContext.Provider>
			</QueryClientProvider>
		),
		root,
	);
	dispose = () => {
		persister?.stop();
		stop();
		root.remove();
	};
	return { api, sent, history, persister, saved };
};

/** A read's answer, for a `read` stub. */
const answered = (data: unknown) => ({ data, headers: new Headers() }) as never;

// Kills a page that looks for the code and the state in its URL, which its
// first script already emptied, rather than in the page's memory.
test("the exchange sends the code and the state the first script kept", async () => {
	const { sent } = open(
		{ code: CODE, state: STATE },
		{ answer: () => new Promise(() => {}) },
	);
	await expect.poll(() => sent.length).toBe(1);
	expect(sent[0]).toEqual({
		method: "POST",
		path: "/v0/github/exchange",
		body: { code: CODE, state: STATE },
	});
});

// Kills a refusal swallowed or shown without the API's message, and a page
// that strands the reader with no way back to the project the state names.
test("a refused exchange shows the API's message and the way back", async () => {
	open(
		{ code: CODE, state: STATE },
		{
			answer: async () => {
				throw new ApiError(
					400,
					"client",
					JSON.stringify({
						message: "The GitHub link state is not valid: ExpiredSignature",
					}),
				);
			},
		},
	);
	await expect
		.element(page.getByRole("alert"))
		.toHaveTextContent(
			"Bencher did not read your GitHub repositories: The GitHub link state is not valid: ExpiredSignature",
		);
	await expect
		.element(page.getByRole("link", { name: "Back to the project" }))
		.toHaveAttribute("href", "/next/console/projects/p/connections");
});

// Kills a return with no code sent to the API as if it had one.
test("a return without a code asks the API nothing", async () => {
	const { sent } = open({ state: STATE }, { answer: async () => EXCHANGED });
	await expect
		.element(page.getByRole("alert"))
		.toHaveTextContent("Bencher did not read your GitHub repositories");
	await expect
		.element(page.getByRole("link", { name: "Back to the project" }))
		.toBeVisible();
	expect(sent).toEqual([]);
});

// Kills a return with no state sent to the API, which would only refuse it.
test("a return without a state asks the API nothing", async () => {
	const { sent } = open({ code: CODE }, { answer: async () => EXCHANGED });
	await expect
		.element(page.getByRole("alert"))
		.toHaveTextContent("Bencher did not read your GitHub repositories");
	expect(sent).toEqual([]);
});

// Kills an exchange that does not open the project's Connections with its
// repositories, and grants kept anywhere the browser stores: the persisted
// query cache, localStorage, or sessionStorage.
test("a return opens Connections with the repositories, and no grant is stored", async () => {
	const { history, persister, saved } = open(
		{ code: CODE, state: STATE },
		{
			answer: async () => EXCHANGED,
			read: async (path) =>
				answered(
					path === "/v0/projects/hashbrown/github" ? {} : { authorize: "" },
				),
			persisted: true,
		},
	);
	await expect
		.element(page.getByRole("button", { name: "Link pompeii-llc/hashbrown" }))
		.toBeVisible();
	expect(history.get()).toBe("/next/console/projects/hashbrown/connections");

	await persister?.flush();
	const stored = saved.join("");
	// The cache saved what the page read, so its store is no empty check.
	expect(stored).toContain(
		JSON.stringify(githubQuery(fakeApi().api, "hashbrown").queryKey),
	);
	expect(stored).not.toContain(GRANT);
	expect(JSON.stringify({ ...localStorage })).not.toContain(GRANT);
	expect(JSON.stringify({ ...sessionStorage })).not.toContain(GRANT);
});

/** A cache store that keeps each save as the text it would write. */
const recording = (saved: string[]): CacheStore => ({
	get: async () => undefined,
	set: async (_reader, value) => {
		saved.push(JSON.stringify(value));
	},
	keys: async () => [],
	delete: async () => {},
	clear: async () => {},
});
