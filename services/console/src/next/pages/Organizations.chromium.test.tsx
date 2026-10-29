import "@bencherdev/ui/styles.css";
import "../styles/console.css";
import { MemoryRouter, Route, createMemoryHistory } from "@solidjs/router";
import { QueryClient, QueryClientProvider } from "@tanstack/solid-query";
import { Suspense } from "solid-js";
import { render } from "solid-js/web";
import { afterEach, describe, expect, test } from "vitest";
import { page, userEvent } from "vitest/browser";
import type {
	JsonAuthUser,
	JsonConsoleUser,
	JsonOrganization,
} from "../../types/bencher";
import { AccountContext } from "../account/context";
import { type Api, ApiError, type Method } from "../api";
import { axeInBothThemes } from "../axe/testing";
import { FlushContext } from "../flush";
import { organizationPath } from "../paths";
import { consoleUserQuery } from "../queries";
import Organizations from "./Organizations";

const READER = {
	user: {
		uuid: "user-uuid",
		name: "Muriel Bagge",
		slug: "muriel-bagge",
		email: "muriel@nowhere.example",
	},
	token: "tok",
} as JsonAuthUser;

const project = (slug: string, organization: string) => ({
	uuid: `${slug}-uuid`,
	name: slug,
	slug,
	organization,
	bmf_version: 1 as const,
	active_alerts: 0,
});

const BOOTSTRAP = {
	uuid: READER.user.uuid,
	name: READER.user.name,
	slug: READER.user.slug,
	organizations: [
		{ uuid: "tw", name: "Tumbleweed Systems", slug: "tumbleweed" },
		{ uuid: "nl", name: "Nowhere Labs", slug: "nowhere-labs" },
		{ uuid: "bf", name: "Bagge Farm", slug: "bagge-farm" },
	],
	projects: [
		project("hashbrown", "nl"),
		project("crinkle-cut", "nl"),
		project("tater", "tw"),
	],
} as JsonConsoleUser;

interface Sent {
	method: Method;
	path: string;
	body: unknown;
}

/** The API, answering the bootstrap with `bootstrap` and each change with `answer`. */
const consoleApi = ({
	bootstrap = async () => BOOTSTRAP,
	answer = async () => ({
		uuid: "new-uuid",
		name: "Bagge Farm",
		slug: "bagge-farm-x1",
	}),
}: {
	bootstrap?: () => Promise<JsonConsoleUser>;
	answer?: (sent: Sent) => Promise<Partial<JsonOrganization>>;
} = {}) => {
	const sent: Sent[] = [];
	const asked: string[] = [];
	const api: Api = {
		get: async <T,>(path: string) => {
			asked.push(path);
			return { data: (await bootstrap()) as T, headers: new Headers() };
		},
		send: async <T,>(method: Method, path: string, body?: unknown) => {
			const change = { method, path, body };
			sent.push(change);
			return { data: (await answer(change)) as T, headers: new Headers() };
		},
	};
	return { api, sent, asked };
};

let dispose: (() => void) | undefined;

afterEach(() => {
	dispose?.();
	dispose = undefined;
	document.body.replaceChildren();
});

const mount = (
	api: Api,
	{
		path = "/organizations",
		client = new QueryClient({ defaultOptions: { queries: { retry: false } } }),
	}: { path?: string; client?: QueryClient } = {},
) => {
	const history = createMemoryHistory();
	history.set({ value: path, replace: true });
	const root = document.createElement("div");
	root.className = "console";
	document.body.append(root);
	/** In order: each flush of the browser's store, then where the page left for. */
	const steps: string[] = [];
	const view = () => (
		<Suspense fallback={<p>Suspended</p>}>
			<Organizations leave={(href) => steps.push(`leave ${href}`)} />
		</Suspense>
	);
	const stop = render(
		() => (
			<QueryClientProvider client={client}>
				<FlushContext.Provider
					value={async () => {
						steps.push("flush");
					}}
				>
					<AccountContext.Provider
						value={{
							api,
							reader: () => READER,
							keepUser: () => {},
							signOut: () => {},
						}}
					>
						<MemoryRouter history={history}>
							<Route path="/organizations" component={view} />
							<Route path="/organizations/add" component={view} />
						</MemoryRouter>
					</AccountContext.Provider>
				</FlushContext.Provider>
			</QueryClientProvider>
		),
		root,
	);
	dispose = () => {
		stop();
		root.remove();
	};
	return { client, history, steps };
};

const rows = () =>
	[...document.querySelectorAll("tbody tr")].map((row) =>
		[...row.querySelectorAll("td")].map((cell) => cell.textContent?.trim()),
	);
const href = (name: string, nth = 0) =>
	page
		.getByRole("link", { name, exact: true })
		.nth(nth)
		.element()
		.getAttribute("href");
const dialog = () => page.getByRole("dialog", { name: "New organization" });
const create = () => page.getByRole("button", { name: "Create organization" });
const newOrganization = () =>
	page.getByRole("button", { name: "New organization" });

describe("the list", () => {
	// Kills organizations in the API's order, projects counted across
	// organizations, an organization with no projects left out, a count that
	// misses one, and New project offered where projects already are.
	test("lists each organization with its projects, and New project where it has none", async () => {
		const { api } = consoleApi();
		mount(api);

		await expect
			.element(page.getByRole("link", { name: "Bagge Farm" }))
			.toBeVisible();
		expect(rows()).toEqual([
			["Bagge Farm", "No projects", "New projectSettings"],
			["Nowhere Labs", "2 projects", "Settings"],
			["Tumbleweed Systems", "1 project", "Settings"],
		]);
		await expect.element(page.getByText("3 organizations")).toBeVisible();
	});

	// Kills an organization that leads anywhere but its General here, and a New
	// project for another organization.
	test("each organization leads to its General, and New project to its own", async () => {
		const { api } = consoleApi();
		mount(api);

		await expect
			.element(page.getByRole("link", { name: "Bagge Farm" }))
			.toBeVisible();
		expect(href("Bagge Farm")).toBe(organizationPath("bagge-farm"));
		expect(href("Settings", 1)).toBe(organizationPath("nowhere-labs"));
		expect(href("New project")).toBe(
			"/console/organizations/bagge-farm/projects/add",
		);
	});

	// Kills a list that waits for the API when the reader's bootstrap is
	// already in the cache, or reads it under another key than the shell's.
	test("paints from the cached bootstrap at once", async () => {
		const { api } = consoleApi({ bootstrap: () => new Promise(() => {}) });
		const client = new QueryClient();
		client.setQueryData(
			consoleUserQuery(api, READER.user.uuid).queryKey,
			BOOTSTRAP,
		);
		mount(api, { client });

		await expect
			.element(page.getByRole("link", { name: "Nowhere Labs" }))
			.toBeVisible();
	});

	// Kills a failed load that leaves a skeleton forever, and a Retry that
	// does not ask again.
	test("a failed load says so and retries", async () => {
		let fail = true;
		const { api, asked } = consoleApi({
			bootstrap: async () => {
				if (fail) {
					throw new ApiError(undefined, "network", "offline");
				}
				return BOOTSTRAP;
			},
		});
		mount(api);

		await expect.element(page.getByRole("alert")).toBeVisible();
		fail = false;
		await page.getByRole("button", { name: "Retry" }).click();

		await expect
			.element(page.getByRole("link", { name: "Bagge Farm" }))
			.toBeVisible();
		expect(asked).toEqual([
			"/v0/console/users/user-uuid",
			"/v0/console/users/user-uuid",
		]);
	});
});

describe("New organization", () => {
	// Kills a slug that does not follow the name, a Create allowed with no
	// name or one over the API's limit, and a dialog that opens elsewhere.
	test("derives the slug from the name, and holds Create back until the name is good", async () => {
		const { api } = consoleApi();
		mount(api);

		await newOrganization().click();
		await expect.element(dialog()).toBeVisible();
		await expect.element(create()).toBeDisabled();
		await page.getByRole("textbox", { name: "Name" }).fill("Café Bagge!");
		await expect.element(dialog()).toHaveTextContent("cafe-bagge");
		await expect.element(create()).toBeEnabled();
		await page.getByRole("textbox", { name: "Name" }).fill("   ");
		await expect.element(create()).toBeDisabled();
		await page.getByRole("textbox", { name: "Name" }).fill("é".repeat(33));
		await expect.element(dialog()).toHaveTextContent("66 of 64 bytes");
		await expect.element(create()).toBeDisabled();
	});

	// Kills a create sent with more than the trimmed name, a landing on the
	// slug the console guessed rather than the one the API made, a bootstrap
	// left without the new organization, and a leave before the store holds it.
	test("creates it, keeps it in the bootstrap, and lands on its New project", async () => {
		const { api, sent } = consoleApi();
		const { client, steps } = mount(api);
		await expect
			.element(page.getByRole("link", { name: "Bagge Farm" }))
			.toBeVisible();

		await newOrganization().click();
		await page.getByRole("textbox", { name: "Name" }).fill(" Bagge Farm ");
		await create().click();

		await expect
			.poll(() => steps)
			.toEqual([
				"flush",
				"leave /console/organizations/bagge-farm-x1/projects/add",
			]);
		expect(sent).toEqual([
			{
				method: "POST",
				path: "/v0/organizations",
				body: { name: "Bagge Farm" },
			},
		]);
		const cached = client.getQueryData<JsonConsoleUser>(
			consoleUserQuery(api, READER.user.uuid).queryKey,
		);
		expect(cached?.organizations).toContainEqual({
			uuid: "new-uuid",
			name: "Bagge Farm",
			slug: "bagge-farm-x1",
		});
	});

	// Kills a second organization made by a second click or Enter while the
	// first is in flight, and a landing that never follows a slow answer.
	test("Create holds while the organization is being made", async () => {
		let answer: (made: Partial<JsonOrganization>) => void = () => {};
		const { api, sent } = consoleApi({
			answer: () => new Promise((resolve) => (answer = resolve)),
		});
		const { steps } = mount(api);

		await newOrganization().click();
		await page.getByRole("textbox", { name: "Name" }).fill("Bagge Farm");
		await create().click();

		await expect.element(create()).toBeDisabled();
		await create().click({ force: true });
		await page.getByRole("textbox", { name: "Name" }).click();
		await userEvent.keyboard("{Enter}");
		expect(sent).toHaveLength(1);
		expect(steps).toEqual([]);

		answer({ uuid: "new-uuid", name: "Bagge Farm", slug: "bagge-farm" });
		await expect
			.poll(() => steps)
			.toEqual([
				"flush",
				"leave /console/organizations/bagge-farm/projects/add",
			]);
		expect(sent).toHaveLength(1);
	});

	// Kills a refusal said nowhere, a dialog that closes or drops the name
	// when refused, and a Create left disabled after it.
	test("a refusal says why and keeps the name", async () => {
		const { api, sent } = consoleApi({
			answer: async () => {
				throw new ApiError(429, "client", '{"message":"Too many requests"}');
			},
		});
		const { steps } = mount(api);

		await newOrganization().click();
		await page.getByRole("textbox", { name: "Name" }).fill("Bagge Farm");
		await create().click();

		await expect
			.element(dialog().getByRole("alert"))
			.toHaveTextContent("Too many requests");
		await expect
			.element(page.getByRole("textbox", { name: "Name" }))
			.toHaveValue("Bagge Farm");
		await expect.element(create()).toBeEnabled();
		expect(sent).toHaveLength(1);
		expect(steps).toEqual([]);
	});

	// Kills an add path that opens the list without the dialog, and a Cancel
	// that leaves the reader on it, where a reload opens the dialog again.
	test("its own path opens it, and closing goes back to the list", async () => {
		const { api } = consoleApi();
		const { history } = mount(api, { path: "/organizations/add" });

		await expect.element(dialog()).toBeVisible();
		await page.getByRole("button", { name: "Cancel" }).click();

		await expect.element(dialog()).not.toBeInTheDocument();
		expect(history.get()).toBe("/organizations");
	});
});

// Kills a New organization dialog that fails axe in either theme.
test("New organization passes axe in both themes", async () => {
	await axeInBothThemes(async () => {
		mount(consoleApi().api, { path: "/organizations/add" });
		await dialog().getByRole("textbox", { name: "Name" }).fill("Bagge Farm");
		await expect.element(dialog()).toHaveTextContent("bagge-farm");
		return () => {
			dispose?.();
			dispose = undefined;
		};
	});
});
