import { afterEach, expect, test } from "vitest";
import { page } from "vitest/browser";
import { type JsonOAuthGrant, OAuthScope } from "../../types/bencher";
import { type Api, ApiError, type Method } from "../api";
import { mountAccount } from "../account/testing";
import { axeInBothThemes } from "../axe/testing";
import ConnectedApps from "./ConnectedApps";

let dispose: (() => void) | undefined;

afterEach(() => {
	dispose?.();
	dispose = undefined;
});

const GRANTS = "/v0/users/muriel-bagge/oauth/grants";
const LIST = `${GRANTS}?per_page=255&direction=desc`;

const chatgpt: JsonOAuthGrant = {
	uuid: "grant-chatgpt",
	client: {
		id: "https://chatgpt.example/oauth/client.json",
		host: "chatgpt.example",
		name: "ChatGPT",
	},
	scopes: [OAuthScope.ProjectsRead, OAuthScope.ProjectsWrite],
	created: "2026-10-02T10:00:00Z",
	modified: "2026-10-02T10:00:00Z",
	expiration: "2026-11-02T10:00:00Z",
} as JsonOAuthGrant;

const claude: JsonOAuthGrant = {
	uuid: "grant-claude",
	client: {
		id: "https://claude.example/oauth/client.json",
		host: "claude.example",
		name: "Claude",
	},
	scopes: [OAuthScope.ProjectsRead],
	user_key: { uuid: "key-uuid", name: "muriel-laptop" },
	created: "2026-09-20T10:00:00Z",
	modified: "2026-10-05T10:00:00Z",
	expiration: "2026-11-20T10:00:00Z",
} as JsonOAuthGrant;

/** The grants API: each list read by `list`, each revoke answered by `answer`. */
const grantsApi = (
	list: () => Promise<{
		grants: JsonOAuthGrant[];
		total?: number;
	}> = async () => ({
		grants: [chatgpt, claude],
	}),
	answer: () => Promise<unknown> = async () => undefined,
) => {
	const reads: string[] = [];
	const sent: { method: Method; path: string }[] = [];
	const api: Api = {
		get: async <T,>(path: string) => {
			reads.push(path);
			const { grants, total } = await list();
			return {
				data: grants as T,
				headers: new Headers({
					"X-Total-Count": String(total ?? grants.length),
				}),
			};
		},
		send: async <T,>(method: Method, path: string) => {
			sent.push({ method, path });
			return { data: (await answer()) as T, headers: new Headers() };
		},
	};
	return { api, reads, sent };
};

const app = (host: string) => page.getByRole("region", { name: host });

// Kills a list read from anywhere but the reader's own grants, oldest first,
// or after another answer, and a page that suspends while it loads.
test("the list starts at once from the reader's grants, under the page's head", async () => {
	const { api, reads } = grantsApi(() => new Promise(() => {}));
	dispose = mountAccount(ConnectedApps, "apps", { api }).dispose;
	await expect.poll(() => reads).toEqual([LIST]);
	await expect
		.element(page.getByRole("heading", { level: 1, name: "Connected Apps" }))
		.toBeVisible();
	expect(page.getByText("Suspended").elements()).toHaveLength(0);
	await expect
		.element(
			page
				.getByRole("navigation", { name: "Account" })
				.getByRole("link", { name: "Connected Apps" }),
		)
		.toHaveAttribute("aria-current", "page");
});

// Kills a field of the classic page left out or swapped between apps: who the
// app is, what it calls itself, what it can do, when it connected and last
// refreshed, and the key it was approved with, which leads to Keys.
test("each app shows who it is, what it can do, and how it was approved", async () => {
	const { api } = grantsApi();
	dispose = mountAccount(ConnectedApps, "apps", { api }).dispose;

	const first = app("chatgpt.example");
	await expect.element(first).toBeVisible();
	expect(page.getByRole("region").elements()).toHaveLength(2);
	await expect
		.element(first.getByRole("heading", { level: 2 }))
		.toHaveTextContent("chatgpt.example");
	await expect.element(first).toHaveTextContent(/calls itself\s*ChatGPT/);
	expect(
		first
			.getByRole("listitem")
			.elements()
			.map((item) => item.textContent),
	).toEqual([
		"View your projects and their results, including private projects",
		"View, create, and change data in your projects, including private projects, such as reports, thresholds, and alerts, and submit runs that create projects or claim unclaimed organizations",
	]);
	const connected = first.element().querySelector("time");
	expect(connected?.getAttribute("datetime")).toBe(chatgpt.created);
	await expect.element(first).toHaveTextContent(/refreshed\s*Never/);
	await expect.element(first).toHaveTextContent(/approved\s*while signed in/);
	expect(first.getByRole("link").elements()).toHaveLength(0);

	const second = app("claude.example");
	expect(
		[...second.element().querySelectorAll("time")].map((time) =>
			time.getAttribute("datetime"),
		),
	).toEqual([claude.created, claude.modified]);
	await expect
		.element(second)
		.toHaveTextContent(/approved\s*with key muriel-laptop/);
	await expect
		.element(second.getByRole("link", { name: "muriel-laptop" }))
		.toHaveAttribute("href", "/console/users/muriel-bagge/keys");
});

// Kills an empty list drawn as nothing, or still waiting.
test("no apps says so", async () => {
	const { api } = grantsApi(async () => ({ grants: [] }));
	dispose = mountAccount(ConnectedApps, "apps", { api }).dispose;
	await expect.element(page.getByText("No connected apps")).toBeVisible();
	expect(page.getByRole("region").elements()).toHaveLength(0);
});

// Kills a list that hides that the API holds more apps than it showed.
test("a list the API cut short says how many it holds", async () => {
	const { api } = grantsApi(async () => ({ grants: [chatgpt], total: 300 }));
	dispose = mountAccount(ConnectedApps, "apps", { api }).dispose;
	await expect.element(page.getByText("Newest 1 of 300")).toBeVisible();
});

// Kills a full list that still says it was cut short.
test("a whole list says nothing of its size", async () => {
	const { api } = grantsApi();
	dispose = mountAccount(ConnectedApps, "apps", { api }).dispose;
	await expect.element(app("claude.example")).toBeVisible();
	expect(page.getByText(/^Newest/).elements()).toHaveLength(0);
});

// Kills a list that waits forever on a failed read, with no way to retry.
test("a list that did not load says so, and Retry reads it again", async () => {
	let fail = true;
	const { api } = grantsApi(async () => {
		if (fail) {
			throw new ApiError(undefined, "network", "Failed to fetch");
		}
		return { grants: [chatgpt] };
	});
	dispose = mountAccount(ConnectedApps, "apps", { api }).dispose;
	await expect
		.element(page.getByRole("alert"))
		.toHaveTextContent("The connected apps did not load");
	fail = false;
	await page.getByRole("button", { name: "Retry" }).click();
	await expect.element(app("chatgpt.example")).toBeVisible();
});

// Kills a server without OAuth offered a Retry that can never work.
test("a server that does not connect apps says so", async () => {
	const { api } = grantsApi(async () => {
		throw new ApiError(404, "not_found", '{"message":"Not Found"}');
	});
	dispose = mountAccount(ConnectedApps, "apps", { api }).dispose;
	await expect
		.element(page.getByText("Not available on this server"))
		.toBeVisible();
	expect(page.getByRole("button", { name: "Retry" }).elements()).toHaveLength(
		0,
	);
	expect(page.getByText("Not Found").elements()).toHaveLength(0);
});

// Kills a revoke sent without asking or on Cancel, sent anywhere but the
// reader's own grant, one that leaves the app listed, and focus left on a
// button that is gone.
test("Revoke asks first: Cancel keeps the app, and Revoke app removes it", async () => {
	const { api, sent } = grantsApi();
	dispose = mountAccount(ConnectedApps, "apps", { api }).dispose;
	const revoke = page.getByRole("button", { name: "Revoke chatgpt.example" });
	await revoke.click();
	const confirm = page.getByRole("alertdialog", {
		name: "Revoke chatgpt.example?",
	});
	await expect
		.element(confirm)
		.toHaveAccessibleDescription(
			"chatgpt.example loses access to this account at once.",
		);
	await confirm.getByRole("button", { name: "Cancel" }).click();
	await expect.element(confirm).not.toBeInTheDocument();
	await expect.element(app("chatgpt.example")).toBeVisible();
	expect(sent).toEqual([]);

	await revoke.click();
	await page
		.getByRole("alertdialog", { name: "Revoke chatgpt.example?" })
		.getByRole("button", { name: "Revoke app" })
		.click();
	expect(sent).toEqual([{ method: "DELETE", path: `${GRANTS}/grant-chatgpt` }]);
	await expect.element(app("chatgpt.example")).not.toBeInTheDocument();
	await expect.element(app("claude.example")).toBeVisible();
	await expect
		.element(page.getByRole("status"))
		.toHaveTextContent("Revoked chatgpt.example");
	await expect
		.element(page.getByRole("heading", { level: 1, name: "Connected Apps" }))
		.toHaveFocus();
});

// Kills a last revoke that leaves an empty page rather than the empty state.
test("revoking the last app shows that none are left", async () => {
	const { api } = grantsApi(async () => ({ grants: [chatgpt] }));
	dispose = mountAccount(ConnectedApps, "apps", { api }).dispose;
	await page.getByRole("button", { name: "Revoke chatgpt.example" }).click();
	await page.getByRole("button", { name: "Revoke app" }).click();
	await expect.element(page.getByText("No connected apps")).toBeVisible();
});

const hosts = () =>
	page
		.getByRole("region")
		.elements()
		.map((region) => region.querySelector("h2")?.textContent);

// Kills a refused revoke that hides the app anyway, says nothing, or puts the
// app back anywhere but where it was.
test("a refused revoke keeps the app in its place and says why", async () => {
	const { api } = grantsApi(undefined, async () => {
		throw new ApiError(500, "server", '{"message":"Database is busy"}');
	});
	dispose = mountAccount(ConnectedApps, "apps", { api }).dispose;
	await expect.element(app("claude.example")).toBeVisible();
	await page.getByRole("button", { name: "Revoke chatgpt.example" }).click();
	await page.getByRole("button", { name: "Revoke app" }).click();
	await expect
		.element(page.getByRole("alert"))
		.toHaveTextContent(
			"Bencher did not revoke chatgpt.example: Database is busy",
		);
	await expect.element(app("chatgpt.example")).toBeVisible();
	expect(hosts()).toEqual(["chatgpt.example", "claude.example"]);
	expect(page.getByText("Revoked chatgpt.example").elements()).toHaveLength(0);
});

// Kills a failure from an earlier revoke left on screen after a later one
// succeeds.
test("a revoke that succeeds clears an earlier refusal", async () => {
	let refuse = true;
	const { api } = grantsApi(undefined, async () => {
		if (refuse) {
			refuse = false;
			throw new ApiError(500, "server", '{"message":"Database is busy"}');
		}
	});
	dispose = mountAccount(ConnectedApps, "apps", { api }).dispose;
	await page.getByRole("button", { name: "Revoke chatgpt.example" }).click();
	await page.getByRole("button", { name: "Revoke app" }).click();
	await expect.element(page.getByRole("alert")).toBeVisible();

	await page.getByRole("button", { name: "Revoke claude.example" }).click();
	await page.getByRole("button", { name: "Revoke app" }).click();
	await expect.element(app("claude.example")).not.toBeInTheDocument();
	await expect
		.element(page.getByRole("status"))
		.toHaveTextContent("Revoked claude.example");
	expect(page.getByRole("alert").elements()).toHaveLength(0);
});

// Kills a revoke, or the undo of a refused one, that leaves the count of a
// cut short list where it was.
test("a revoke counts the app out, and a refused one counts it back", async () => {
	let reads = 0;
	let refuse = true;
	const { api } = grantsApi(
		async () => {
			reads += 1;
			// The read after a revoke stays in flight, so the count shown is the page's own.
			return reads === 1
				? { grants: [chatgpt, claude], total: 300 }
				: new Promise(() => {});
		},
		async () => {
			if (refuse) {
				throw new ApiError(500, "server", '{"message":"Database is busy"}');
			}
		},
	);
	dispose = mountAccount(ConnectedApps, "apps", { api }).dispose;
	await expect.element(page.getByText("Newest 2 of 300")).toBeVisible();
	await page.getByRole("button", { name: "Revoke chatgpt.example" }).click();
	await page.getByRole("button", { name: "Revoke app" }).click();
	await expect.element(page.getByRole("alert")).toBeVisible();
	await expect.element(page.getByText("Newest 2 of 300")).toBeVisible();

	refuse = false;
	await page.getByRole("button", { name: "Revoke chatgpt.example" }).click();
	await page.getByRole("button", { name: "Revoke app" }).click();
	await expect.element(page.getByText("Newest 1 of 299")).toBeVisible();
});

// Kills a cut short list emptied by revokes that claims no apps are left
// while the API holds more, and one that never shows them.
test("a cut short list reads the apps past it once a revoke makes room", async () => {
	let reads = 0;
	const { api } = grantsApi(async () => {
		reads += 1;
		return reads === 1
			? { grants: [chatgpt], total: 300 }
			: { grants: [claude], total: 299 };
	});
	dispose = mountAccount(ConnectedApps, "apps", { api }).dispose;
	await page.getByRole("button", { name: "Revoke chatgpt.example" }).click();
	await page.getByRole("button", { name: "Revoke app" }).click();
	await expect.element(app("claude.example")).toBeVisible();
	await expect.element(page.getByText("Newest 1 of 299")).toBeVisible();
	expect(page.getByText("No connected apps").elements()).toHaveLength(0);
});

// Kills a cut short list emptied by revokes that shows the empty state, or
// a count of none, while its next read is in flight.
test("an emptied cut short list waits for its next read", async () => {
	let reads = 0;
	const { api } = grantsApi(async () => {
		reads += 1;
		return reads === 1
			? { grants: [chatgpt], total: 300 }
			: new Promise(() => {});
	});
	dispose = mountAccount(ConnectedApps, "apps", { api }).dispose;
	await page.getByRole("button", { name: "Revoke chatgpt.example" }).click();
	await page.getByRole("button", { name: "Revoke app" }).click();
	await expect.element(app("chatgpt.example")).not.toBeInTheDocument();
	await expect.poll(() => reads).toBe(2);
	expect(page.getByText("No connected apps").elements()).toHaveLength(0);
	expect(page.getByText(/^Newest/).elements()).toHaveLength(0);
});

// Kills an app revoked elsewhere, which the API no longer finds, called a
// failure and put back.
test("an app already revoked elsewhere is just gone", async () => {
	const { api } = grantsApi(undefined, async () => {
		throw new ApiError(404, "not_found", '{"message":"Not Found"}');
	});
	dispose = mountAccount(ConnectedApps, "apps", { api }).dispose;
	await page.getByRole("button", { name: "Revoke chatgpt.example" }).click();
	await page.getByRole("button", { name: "Revoke app" }).click();
	await expect.element(app("chatgpt.example")).not.toBeInTheDocument();
	expect(page.getByRole("alert").elements()).toHaveLength(0);
});

// Kills a revoke confirmation that fails axe in either theme.
test("the revoke confirmation passes axe in both themes", async () => {
	await axeInBothThemes(async () => {
		dispose = mountAccount(ConnectedApps, "apps", {
			api: grantsApi().api,
		}).dispose;
		await page.getByRole("button", { name: "Revoke chatgpt.example" }).click();
		await expect
			.element(
				page.getByRole("alertdialog", { name: "Revoke chatgpt.example?" }),
			)
			.toBeVisible();
		return () => {
			dispose?.();
			dispose = undefined;
		};
	});
});
