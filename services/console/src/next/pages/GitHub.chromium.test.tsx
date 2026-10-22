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
	type JsonProjectGitHub,
	LinkedRepositoryReason,
	LinkedRepositoryStatus,
} from "../../types/bencher";
import { AccountContext } from "../account/context";
import { ApiError } from "../api";
import { RETURN_KEY } from "../astro/githubReturn";
import { type CacheStore, persistCache } from "../cache";
import { CONSOLE_PATH, PROJECTS_PATH } from "../paths";
import { ProjectContext } from "../project";
import Connections, {
	githubQuery,
	hold,
	recheck,
} from "../settings/Connections";
import { type Sent, bootstrapOf, fakeApi } from "../settings/testing";
import GitHub from "./GitHub";

let dispose: (() => void) | undefined;
const RECHECK = recheck.after;

afterEach(() => {
	dispose?.();
	dispose = undefined;
	hold(undefined);
	recheck.after = RECHECK;
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
		cached = [],
	}: {
		answer: (sent: Sent) => Promise<unknown>;
		read?: (path: string) => Promise<never>;
		persisted?: boolean;
		/** What the cache already holds from before the reader left for GitHub, by query key. */
		cached?: [unknown[], unknown][];
	},
) => {
	const { api, sent } = fakeApi(answer, read);
	const client = new QueryClient({
		defaultOptions: {
			queries: { retry: false, staleTime: Number.POSITIVE_INFINITY },
		},
	});
	client.setQueryData(["console", "project", "hashbrown"], bootstrapOf());
	for (const [key, data] of cached) {
		client.setQueryData(key, data);
	}
	const saved: string[] = [];
	const persister = persisted
		? persistCache(client, recording(saved), READER.user.uuid, "build", {
				delay: 0,
			})
		: undefined;
	Reflect.set(window, RETURN_KEY, kept);
	const history = createMemoryHistory();
	history.set({ value: `${CONSOLE_PATH}/github`, replace: true });
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
					<MemoryRouter history={history} base={CONSOLE_PATH}>
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
		.toHaveAttribute("href", `${PROJECTS_PATH}/p/connections`);
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
	expect(history.get()).toBe(`${PROJECTS_PATH}/hashbrown/connections`);

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

const LINK = "/v0/projects/hashbrown/github";
const DISCONNECTED: JsonProjectGitHub = {
	linked_repository: {
		full_name: "pompeii-llc/hashbrown",
		status: LinkedRepositoryStatus.Disconnected,
		reason: LinkedRepositoryReason.Uninstalled,
		user: { uuid: "u", name: "Muriel Bagge", slug: "muriel-bagge" },
		linked: "2026-09-02T10:00:00Z",
	},
};
const CONNECTED: JsonProjectGitHub = {
	linked_repository: {
		...(DISCONNECTED.linked_repository as NonNullable<
			JsonProjectGitHub["linked_repository"]
		>),
		status: LinkedRepositoryStatus.Connected,
		reason: undefined,
	},
};
const card = () => page.getByRole("region", { name: "GitHub" });

// Kills a return that shows the link the cache held from before the reader
// left for GitHub, where an install reconnected it in the meantime.
test("a return reads the link again rather than the one cached before it", async () => {
	open(
		{ code: CODE, state: STATE },
		{
			answer: async () => EXCHANGED,
			read: async (path) =>
				answered(path === LINK ? CONNECTED : { authorize: "" }),
			cached: [
				[githubQuery(fakeApi().api, "hashbrown").queryKey, DISCONNECTED],
			],
		},
	);
	await expect
		.element(page.getByRole("button", { name: "Link pompeii-llc/hashbrown" }))
		.toBeVisible();
	await expect.element(card()).toHaveTextContent("Connected");
	await expect.element(card()).not.toHaveTextContent("Disconnected");
});

// Kills a return that reads the link only before GitHub tells the API about
// the installation, which can land seconds after the reader is back, and a
// card that keeps reading it after that once.
test("a link the API reconnects soon after the return shows as connected", async () => {
	recheck.after = 250;
	let reads = 0;
	open(
		{ code: CODE, state: STATE },
		{
			answer: async () => EXCHANGED,
			read: async (path) => {
				if (path !== LINK) {
					return answered({ authorize: "" });
				}
				reads += 1;
				return answered(reads === 1 ? DISCONNECTED : CONNECTED);
			},
		},
	);
	await expect.element(card()).toHaveTextContent("Disconnected");
	await expect.element(card()).not.toHaveTextContent("Disconnected");
	await expect.element(card()).toHaveTextContent("Connected");
	await new Promise((resolve) => setTimeout(resolve, 3 * recheck.after));
	expect(reads).toBe(2);
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
