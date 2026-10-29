import { afterEach, expect, test } from "vitest";
import { page } from "vitest/browser";
import type { JsonUserKey, JsonUserKeyCreated } from "../../types/bencher";
import { type Api, ApiError, type Method } from "../api";
import { mountAccount } from "../account/testing";
import { axeInBothThemes } from "../axe/testing";
import UserKeys from "./UserKeys";

let dispose: (() => void) | undefined;

afterEach(() => {
	dispose?.();
	dispose = undefined;
});

const KEYS = "/v0/users/muriel-bagge/keys";

const laptop: JsonUserKey = {
	uuid: "key-uuid",
	user: "user-uuid",
	name: "muriel-laptop",
	creation: "2026-10-02T10:00:00Z",
	expiration: "2026-12-31T10:00:00Z",
};

const created: JsonUserKeyCreated = {
	uuid: "new-uuid",
	user: "user-uuid",
	name: "deploy-bot",
	key: "bencher_user_secret",
	creation: "2026-10-17T00:00:00Z",
	expiration: "2027-01-15T00:00:00Z",
};

/** The user keys API: each list read by `list`, each change answered by `answer`. */
const keysApi = (
	list: (path: string) => Promise<JsonUserKey[]> = async (path) =>
		path.includes("revoked=true") ? [] : [laptop],
	answer: () => Promise<unknown> = async () => undefined,
) => {
	const reads: string[] = [];
	const sent: { method: Method; path: string }[] = [];
	const api: Api = {
		get: async <T,>(path: string) => {
			reads.push(path);
			const keys = await list(path);
			return {
				data: keys as T,
				headers: new Headers({ "X-Total-Count": String(keys.length) }),
			};
		},
		send: async <T,>(method: Method, path: string) => {
			sent.push({ method, path });
			return { data: (await answer()) as T, headers: new Headers() };
		},
	};
	return { api, reads, sent };
};

// Kills lists read from anywhere but the reader's own keys, lists that wait
// on another answer before they start, and a page that suspends while they load.
test("both lists start at once from the user's keys, under the page's head", async () => {
	const { api, reads } = keysApi(() => new Promise(() => {}));
	dispose = mountAccount(UserKeys, "keys", { api }).dispose;
	await expect
		.poll(() => [...reads].sort())
		.toEqual([
			`${KEYS}?revoked=false&per_page=255`,
			`${KEYS}?revoked=true&per_page=255`,
		]);
	await expect
		.element(page.getByRole("heading", { level: 1, name: "Keys" }))
		.toBeVisible();
	await expect
		.element(page.getByRole("button", { name: "New key" }))
		.toBeVisible();
	expect(page.getByText("Suspended").elements()).toHaveLength(0);
	await expect
		.element(
			page
				.getByRole("navigation", { name: "Account" })
				.getByRole("link", { name: "Keys" }),
		)
		.toHaveAttribute("aria-current", "page");
});

// Kills a list that waits forever on a failed read, with no way to retry.
test("lists that did not load say so, and Retry reads them again", async () => {
	let fail = true;
	const { api } = keysApi(async (path) => {
		if (fail) {
			throw new ApiError(undefined, "network", "Failed to fetch");
		}
		return path.includes("revoked=true") ? [] : [laptop];
	});
	dispose = mountAccount(UserKeys, "keys", { api }).dispose;
	await expect
		.element(page.getByRole("alert"))
		.toHaveTextContent("The keys did not load");
	fail = false;
	await page.getByRole("button", { name: "Retry" }).click();
	await expect
		.element(
			page
				.getByRole("table", { name: "Active keys" })
				.getByText("muriel-laptop"),
		)
		.toBeVisible();
});

// Kills a key made anywhere but the reader's own keys, a secret shown again
// or kept in the cache, and a new key missing from the list.
test("New key makes a user key and shows its secret once", async () => {
	const { api, sent } = keysApi(undefined, async () => created);
	const mounted = mountAccount(UserKeys, "keys", { api });
	dispose = mounted.dispose;
	await page.getByRole("button", { name: "New key" }).click();
	await page.getByRole("textbox", { name: "Name" }).fill("deploy-bot");
	await page.getByRole("button", { name: "Create key" }).click();
	expect(sent).toEqual([{ method: "POST", path: KEYS }]);

	const reveal = page.getByRole("dialog", { name: "Copy your new key" });
	await expect.element(reveal).toHaveTextContent("bencher_user_secret");
	await reveal.getByRole("button", { name: "Done" }).click();
	await expect.element(reveal).not.toBeInTheDocument();
	await expect
		.element(
			page.getByRole("table", { name: "Active keys" }).getByText("deploy-bot"),
		)
		.toBeVisible();
	expect(page.getByText("bencher_user_secret").elements()).toHaveLength(0);
	const cached = mounted.client
		.getQueryCache()
		.getAll()
		.map(({ state }) => state.data);
	expect(JSON.stringify(cached)).not.toContain("bencher_user_secret");
});

// Kills a revoke sent without asking, sent anywhere but the reader's own
// keys, or one that leaves the key among the active ones.
test("Revoke asks first, then revokes the user key", async () => {
	const { api, sent } = keysApi();
	dispose = mountAccount(UserKeys, "keys", { api }).dispose;
	await page.getByRole("button", { name: "Revoke muriel-laptop" }).click();
	const confirm = page.getByRole("alertdialog", {
		name: "Revoke muriel-laptop?",
	});
	await expect.element(confirm).toBeVisible();
	expect(sent).toEqual([]);

	await confirm.getByRole("button", { name: "Revoke key" }).click();
	expect(sent).toEqual([{ method: "DELETE", path: `${KEYS}/key-uuid` }]);
	await expect
		.element(page.getByRole("radio", { name: "Active 0" }))
		.toBeInTheDocument();
	await page.getByRole("radio", { name: /^Revoked/ }).click();
	await expect
		.element(
			page
				.getByRole("table", { name: "Revoked keys" })
				.getByText("muriel-laptop"),
		)
		.toBeVisible();
});

// Kills a new key dialog that fails axe in either theme.
test("the new key dialog passes axe in both themes", async () => {
	await axeInBothThemes(async () => {
		dispose = mountAccount(UserKeys, "keys", { api: keysApi().api }).dispose;
		await page.getByRole("button", { name: "New key" }).click();
		await expect
			.element(page.getByRole("dialog", { name: "New key" }))
			.toBeVisible();
		return () => {
			dispose?.();
			dispose = undefined;
		};
	});
});
