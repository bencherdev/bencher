import "../public/public.css";
import { afterEach, expect, test } from "vitest";
import { page, userEvent } from "vitest/browser";
import type { Api } from "../api";
import { DEFAULT_SEARCH } from "../reports/search";
import { bootstrapOf, fakeApi, mount } from "../settings/testing";
import { batchKey, batchQuery } from "./query";
import Setup from "./Setup";

let dispose: (() => void) | undefined;
afterEach(() => {
	dispose?.();
	dispose = undefined;
});

const CREATED = {
	uuid: "key-uuid",
	project: "p",
	name: "bencher run",
	creation: "2026-09-02T10:00:00Z",
	expiration: "2026-12-01T10:00:00Z",
	key: "bencher_run_secret",
};
const ACTIVE = ["console", "keys", "hashbrown", "active"];
const BATCH = { page: 1, perPage: 8 };
/** The card, polling the Runs list's first batch, which holds no run. */
const card = (api: Api) => (
	<Setup poll={() => batchQuery(api, "hashbrown", DEFAULT_SEARCH, BATCH)} />
);
const NO_RUNS: [unknown[], unknown] = [
	[...batchKey("hashbrown", DEFAULT_SEARCH, BATCH)],
	{ runs: [], total: 0, batch: BATCH },
];

// Kills a key made with another name or no expiry, a secret kept in the cache
// or shown with Create key still offered, and a Keys list left without the key.
test("Create key makes a key, shows it once with its export line, and lists it", async () => {
	const { api, sent } = fakeApi(async () => CREATED);
	const mounted = mount(() => card(api), {
		api,
		cached: [[ACTIVE, { keys: [], total: 0 }], NO_RUNS],
	});
	dispose = mounted.dispose;
	await userEvent.click(page.getByRole("button", { name: "Create key" }));
	await expect
		.element(page.getByRole("status", { name: "Your new project key" }))
		.toHaveTextContent("bencher_run_secret");
	await expect
		.element(page.getByText("export BENCHER_API_KEY=bencher_run_secret"))
		.toBeVisible();
	await expect
		.element(page.getByRole("button", { name: "Create key" }))
		.not.toBeInTheDocument();
	expect(sent).toEqual([
		{
			method: "POST",
			path: "/v0/projects/hashbrown/keys",
			body: { name: "bencher run", ttl: 90 * 24 * 60 * 60 },
		},
	]);
	expect(mounted.client.getQueryState(ACTIVE)?.isInvalidated).toBe(true);
	expect(
		JSON.stringify(
			mounted.client
				.getQueryCache()
				.getAll()
				.map(({ state }) => state.data),
		),
	).not.toContain("bencher_run_secret");
});

// Kills a refused key that says nothing or leaves Create key stuck.
test("a refused key says so and Create key stays", async () => {
	const { api } = fakeApi(async () => {
		throw new Error("refused");
	});
	dispose = mount(() => card(api), { api, cached: [NO_RUNS] }).dispose;
	await userEvent.click(page.getByRole("button", { name: "Create key" }));
	await expect
		.element(page.getByRole("alert"))
		.toHaveTextContent("Bencher did not make the key");
	await expect
		.element(page.getByRole("button", { name: "Create key" }))
		.toBeEnabled();
});

// Kills Create key offered to a reader the API refuses it to.
test("a reader who is not a Maintainer gets the card without Create key", async () => {
	const { api } = fakeApi();
	dispose = mount(() => card(api), {
		api,
		bootstrap: bootstrapOf({ manage: false }),
		cached: [NO_RUNS],
	}).dispose;
	await expect
		.element(page.getByText("Read only", { exact: true }))
		.toBeVisible();
	await expect
		.element(page.getByRole("button", { name: "Create key" }))
		.not.toBeInTheDocument();
});
