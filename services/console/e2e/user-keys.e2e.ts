import type { Page } from "@playwright/test";
import { createUserKey, unique } from "./api";
import { axeInBothThemes } from "./axe";
import { expect, seed, settle, signedIn, test } from "./fixtures";

const { slug } = seed.member.user;
const keysPath = `/console/users/${slug}/keys`;
const tokensPath = `/console/users/${slug}/tokens`;
const TOKENS = `${seed.api_url}/v0/users/${slug}/tokens`;

const status = (page: Page) => page.getByRole("main").getByRole("status");

/** What this browser stores: local storage and the console's cache. */
const stored = (page: Page) =>
	page.evaluate(async () => {
		const local = JSON.stringify({ ...localStorage });
		const cache = await new Promise<string>((resolve) => {
			const open = indexedDB.open("bencher-console");
			open.onsuccess = () => {
				const read = open.result
					.transaction("cache")
					.objectStore("cache")
					.getAll();
				read.onsuccess = () => resolve(JSON.stringify(read.result));
				read.onerror = () => resolve("");
			};
			open.onerror = () => resolve("");
		});
		return local + cache;
	});

const LEGACY = [
	{
		uuid: "00000000-0000-4000-8000-00000000c120",
		user: seed.member.user.uuid,
		name: "ci-2024",
		token: "legacy-secret-ci",
		creation: "2024-03-03T00:00:00Z",
		expiration: "2027-03-03T00:00:00Z",
	},
	{
		uuid: "00000000-0000-4000-8000-0000000d0e57",
		user: seed.member.user.uuid,
		name: "home-desktop",
		token: "legacy-secret-desktop",
		creation: "2023-11-20T00:00:00Z",
		expiration: "2028-11-20T00:00:00Z",
	},
];

/**
 * The API lists two legacy tokens for the member and counts them, since no
 * API makes one any more; every other request, a revoke included, goes on.
 */
const twoLegacyTokens = (page: Page) =>
	page.route(`${TOKENS}?*`, async (route) => {
		if (route.request().method() !== "GET") {
			return route.fallback();
		}
		const response = await route.fetch();
		const { "content-length": _, ...headers } = response.headers();
		const list = new URL(route.request().url()).searchParams.has("revoked");
		await route.fulfill({
			response,
			json: list ? LEGACY : await response.json(),
			headers: { ...headers, "x-total-count": "2" },
		});
	});

test.describe("Keys", () => {
	test.use({ storageState: signedIn(seed.member) });

	// Kills a reveal that can be shown twice, a secret kept anywhere the browser
	// stores, a new key missing from the list, and a rail that leaves the app.
	test("a new key is shown once, and is gone after navigation", async ({
		page,
	}) => {
		const name = unique("deploy-bot");
		await page.goto(keysPath);
		await page.getByRole("button", { name: "New key" }).click();
		const form = page.getByRole("dialog", { name: "New key" });
		await form.getByRole("textbox", { name: "Name" }).fill(name);
		await form.getByRole("button", { name: "Create key" }).click();

		const reveal = page.getByRole("dialog", { name: "Copy your new key" });
		await expect(reveal).toContainText(name);
		const secret = (
			await reveal.getByText(/^bencher_user_\w+$/).textContent()
		)?.trim();
		expect(secret).toMatch(/^bencher_user_\w+$/);
		await reveal.getByRole("button", { name: "Done" }).click();
		await expect(reveal).toBeHidden();

		const active = page.getByRole("table", { name: "Active keys" });
		await expect(
			active.getByRole("row", { name: new RegExp(name) }),
		).toBeVisible();
		await expect(page.getByText(secret ?? "")).toHaveCount(0);

		await page.evaluate(() => {
			(window as Window & { stayed?: boolean }).stayed = true;
		});
		const rail = page.getByRole("navigation", { name: "Account" });
		await rail.getByRole("link", { name: "Profile" }).click();
		await expect(
			page.getByRole("heading", { level: 1, name: "Profile" }),
		).toBeVisible();
		await rail.getByRole("link", { name: "Keys" }).click();
		await expect(
			active.getByRole("row", { name: new RegExp(name) }),
		).toBeVisible();
		expect(
			await page.evaluate(
				() => (window as Window & { stayed?: boolean }).stayed,
			),
		).toBe(true);

		await settle(page, 1_500);
		const kept = await stored(page);
		expect(kept).toContain(name);
		expect(kept).not.toContain(secret);
	});

	// Kills a revoke without its confirmation, one that only hides the row, and
	// one that a reload undoes.
	test("Revoke asks first and moves the key to Revoked", async ({
		page,
		request,
	}) => {
		const name = unique("old-laptop");
		await createUserKey(request, name);
		await page.goto(keysPath);
		const active = page.getByRole("table", { name: "Active keys" });
		await expect(
			active.getByRole("row", { name: new RegExp(name) }),
		).toBeVisible();

		await page.getByRole("button", { name: `Revoke ${name}` }).click();
		const confirm = page.getByRole("alertdialog", { name: `Revoke ${name}?` });
		await confirm.getByRole("button", { name: "Revoke key" }).click();
		await expect(status(page)).toHaveText(`Revoked ${name}`);
		await expect(
			active.getByRole("row", { name: new RegExp(name) }),
		).toHaveCount(0);

		await settle(page);
		await page.reload();
		await page.getByRole("radio", { name: /^Revoked/ }).check();
		await expect(
			page
				.getByRole("table", { name: "Revoked keys" })
				.getByRole("row", { name: new RegExp(name) }),
		).toBeVisible();
	});
});

test.describe("Legacy tokens", () => {
	test.use({ storageState: signedIn(seed.member) });

	// Kills a Profile link that leaves the app, a list that keeps a token's
	// secret, a revoke sent without asking or to another path, and a refused
	// revoke that hides the token anyway. No API makes a legacy token any more,
	// so the list is the API's answer rewritten, and the real API refuses to
	// revoke a token it never made.
	test("Profile's link lists them, and a refused revoke keeps the token", async ({
		page,
	}) => {
		await twoLegacyTokens(page);
		await page.goto(`/console/users/${slug}/settings`);
		await page.evaluate(() => {
			(window as Window & { stayed?: boolean }).stayed = true;
		});
		await page.getByRole("link", { name: "2 legacy tokens" }).click();

		await expect(page).toHaveURL(tokensPath);
		const table = page.getByRole("table", { name: "Legacy API tokens" });
		await expect(table.getByRole("row", { name: /ci-2024/ })).toBeVisible();
		expect(
			await page.evaluate(
				() => (window as Window & { stayed?: boolean }).stayed,
			),
		).toBe(true);

		await page.getByRole("button", { name: "Revoke ci-2024" }).click();
		const confirm = page.getByRole("alertdialog", { name: "Revoke ci-2024?" });
		const revoked = page.waitForRequest(
			(sent) =>
				sent.method() === "DELETE" &&
				sent.url() === `${TOKENS}/${LEGACY[0]?.uuid}`,
		);
		await confirm.getByRole("button", { name: "Revoke token" }).click();
		await revoked;
		await expect(page.getByRole("alert")).toContainText(
			"Bencher did not revoke ci-2024",
		);
		await expect(table.getByRole("row", { name: /ci-2024/ })).toBeVisible();

		await settle(page, 1_500);
		expect(await stored(page)).not.toContain("legacy-secret");
	});
});

test.describe("accessibility", () => {
	test.use({ storageState: signedIn(seed.member) });

	// Kills contrast, names, and roles that fail axe in either theme.
	test("Keys passes axe in both themes", async ({ page }) => {
		await axeInBothThemes(page, keysPath, () =>
			expect(page.getByRole("table", { name: "Active keys" })).toBeVisible(),
		);
	});

	test("Legacy tokens passes axe in both themes", async ({ page }) => {
		await twoLegacyTokens(page);
		await axeInBothThemes(page, tokensPath, () =>
			expect(
				page.getByRole("button", { name: "Revoke ci-2024" }),
			).toBeVisible(),
		);
	});
});
