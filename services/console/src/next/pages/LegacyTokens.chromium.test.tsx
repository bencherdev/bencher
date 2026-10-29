import { afterEach, expect, test } from "vitest";
import { page } from "vitest/browser";
import type { JsonToken } from "../../types/bencher";
import { type Api, ApiError, type Method } from "../api";
import { mountAccount } from "../account/testing";
import { axeInBothThemes } from "../axe/testing";
import LegacyTokens from "./LegacyTokens";

let dispose: (() => void) | undefined;

afterEach(() => {
	dispose?.();
	dispose = undefined;
});

const TOKENS = "/v0/users/muriel-bagge/tokens";
const COUNT = ["account", "tokens", "muriel-bagge"];

const token = (name: string, creation: string): JsonToken => ({
	uuid: `uuid-${name}`,
	user: "user-uuid",
	name,
	token: `jwt-of-${name}`,
	creation,
	expiration: "2028-11-20T00:00:00Z",
});
const ci = token("ci-2024", "2024-03-03T00:00:00Z");
const desktop = token("home-desktop", "2023-11-20T00:00:00Z");

/** The tokens API: the list read by `list`, each change answered by `answer`. */
const tokensApi = (
	answer: () => Promise<unknown> = async () => undefined,
	list: () => Promise<JsonToken[]> = async () => [desktop, ci],
) => {
	const reads: string[] = [];
	const sent: { method: Method; path: string }[] = [];
	const api: Api = {
		get: async <T,>(path: string) => {
			reads.push(path);
			const tokens = await list();
			return {
				data: tokens as T,
				headers: new Headers({ "X-Total-Count": String(tokens.length) }),
			};
		},
		send: async <T,>(method: Method, path: string) => {
			sent.push({ method, path });
			return { data: (await answer()) as T, headers: new Headers() };
		},
	};
	return { api, reads, sent };
};

const table = () => page.getByRole("table", { name: "Legacy API tokens" });
const rows = () =>
	[...document.querySelectorAll("tbody tr")].map((row) => row.textContent);

// Kills revoked tokens listed, a list in the API's name order, a Create or
// status toggle drawn, a rail that does not mark Profile, and a token's
// secret kept in the cache.
test("lists the unrevoked tokens newest first, with Revoke alone", async () => {
	const { api, reads } = tokensApi();
	const mounted = mountAccount(LegacyTokens, "tokens", { api });
	dispose = mounted.dispose;
	await expect.element(table().getByText("ci-2024")).toBeVisible();
	expect(reads).toEqual([`${TOKENS}?revoked=false&per_page=255`]);
	expect(rows()[0]).toContain("ci-2024");
	expect(rows()[1]).toContain("home-desktop");
	expect(
		page.getByRole("button", { name: "Revoke ci-2024" }).elements(),
	).toHaveLength(1);
	expect(page.getByRole("button", { name: /New/ }).elements()).toHaveLength(0);
	expect(page.getByRole("radiogroup").elements()).toHaveLength(0);
	await expect
		.element(
			page
				.getByRole("navigation", { name: "Account" })
				.getByRole("link", { name: "Profile" }),
		)
		.toHaveAttribute("aria-current", "page");
	const cached = mounted.client
		.getQueryCache()
		.getAll()
		.map(({ state }) => state.data);
	expect(JSON.stringify(cached)).not.toContain("jwt-of-");
});

// Kills a revoke sent without asking or to another path, a row left in the
// list, Profile's count left stale, and focus dropped with the removed row.
test("Revoke asks first, then the token leaves, Profile's count drops, and focus stays on the page", async () => {
	const { api, sent } = tokensApi();
	const mounted = mountAccount(LegacyTokens, "tokens", {
		api,
		cached: [[COUNT, 2]],
	});
	dispose = mounted.dispose;
	await page.getByRole("button", { name: "Revoke ci-2024" }).click();
	const confirm = page.getByRole("alertdialog", { name: "Revoke ci-2024?" });
	await expect.element(confirm).toBeVisible();
	expect(sent).toEqual([]);

	await confirm.getByRole("button", { name: "Revoke token" }).click();
	expect(sent).toEqual([{ method: "DELETE", path: `${TOKENS}/uuid-ci-2024` }]);
	await expect.element(table().getByText("ci-2024")).not.toBeInTheDocument();
	await expect.element(page.getByRole("status")).toHaveTextContent("Revoked");
	await expect.poll(() => mounted.client.getQueryData(COUNT)).toBe(1);
	await expect
		.element(page.getByRole("heading", { level: 1, name: "Legacy tokens" }))
		.toHaveFocus();
});

// Kills a refused revoke that leaves the token gone, or Profile's count
// dropped, or says nothing.
test("a refused revoke puts the token back and says why", async () => {
	const { api } = tokensApi(async () => {
		throw new ApiError(404, "client", '{"message":"Token not found"}');
	});
	const mounted = mountAccount(LegacyTokens, "tokens", {
		api,
		cached: [[COUNT, 2]],
	});
	dispose = mounted.dispose;
	await page.getByRole("button", { name: "Revoke ci-2024" }).click();
	await page
		.getByRole("alertdialog", { name: "Revoke ci-2024?" })
		.getByRole("button", { name: "Revoke token" })
		.click();
	await expect
		.element(page.getByRole("alert"))
		.toHaveTextContent("Not revoked: Token not found");
	await expect.element(table().getByText("ci-2024")).toBeVisible();
	expect(mounted.client.getQueryData(COUNT)).toBe(2);
});

// Kills a list that waits forever on a failed read, with no way to retry.
test("a list that did not load says so, and Retry reads it again", async () => {
	let fail = true;
	const { api } = tokensApi(undefined, async () => {
		if (fail) {
			throw new ApiError(undefined, "network", "Failed to fetch");
		}
		return [ci];
	});
	dispose = mountAccount(LegacyTokens, "tokens", { api }).dispose;
	await expect
		.element(page.getByRole("alert"))
		.toHaveTextContent("The legacy tokens did not load");
	fail = false;
	await page.getByRole("button", { name: "Retry" }).click();
	await expect.element(table().getByText("ci-2024")).toBeVisible();
});

// Kills a revoke confirmation that fails axe in either theme.
test("the revoke confirmation passes axe in both themes", async () => {
	await axeInBothThemes(async () => {
		dispose = mountAccount(LegacyTokens, "tokens", {
			api: tokensApi().api,
		}).dispose;
		await page.getByRole("button", { name: "Revoke ci-2024" }).click();
		await expect
			.element(page.getByRole("alertdialog", { name: "Revoke ci-2024?" }))
			.toBeVisible();
		return () => {
			dispose?.();
			dispose = undefined;
		};
	});
});
