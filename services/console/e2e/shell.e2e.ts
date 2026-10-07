import type { Page, Route } from "@playwright/test";
import {
	crumbs,
	expect,
	nextPath,
	seed,
	settle,
	signedIn,
	tabRow,
	test,
} from "./fixtures";

const { hashbrown } = seed.projects;
const TABS = [
	"Explore",
	"Plots",
	"Reports",
	"Alerts",
	"Thresholds",
	"Settings",
];

test.describe("without JavaScript", () => {
	test.use({ javaScriptEnabled: false, storageState: signedIn(seed.member) });

	// Kills a shell that needs JavaScript to draw, and a current tab taken from
	// anything but the path.
	test("a cold deep link draws the bar and the tab row with Reports current", async ({
		page,
	}) => {
		await page.goto(nextPath(hashbrown.slug, "reports"));

		await expect(
			page.getByRole("banner").getByRole("link", { name: "Bencher, home" }),
		).toBeVisible();
		await expect(crumbs(page)).toContainText(hashbrown.slug);
		for (const tab of TABS) {
			await expect(
				tabRow(page).getByRole("link", { name: tab, exact: true }),
			).toBeVisible();
		}
		await expect(tabRow(page).locator('[aria-current="page"]')).toHaveText(
			"Reports",
		);
	});
});

test.describe("signed in", () => {
	test.use({ storageState: signedIn(seed.member) });

	// Kills crumbs drawn from the slug, and an Alerts count that is not the
	// active alerts of this project (the seed also holds a dismissed one).
	test("the crumbs name the organization and the project, and Alerts counts the active alerts", async ({
		page,
	}) => {
		await page.goto(nextPath(hashbrown.slug, "reports"));

		await expect(
			crumbs(page).getByRole("link", { name: seed.organization.name }),
		).toBeVisible();
		await expect(
			crumbs(page).getByRole("link", { name: hashbrown.name }),
		).toBeVisible();
		await expect(
			tabRow(page).getByRole("link", { name: "Alerts 1 active" }),
		).toBeVisible();
	});

	// Kills tab links that reload the document, shell data fetched per page,
	// and a client-side navigation a screen reader does not hear.
	test("switching tabs routes on the client and fetches nothing already cached", async ({
		page,
	}) => {
		await page.goto(nextPath(hashbrown.slug, "reports"));
		await expect(
			tabRow(page).getByRole("link", { name: "Alerts 1 active" }),
		).toBeVisible();
		await settle(page);

		const documents: string[] = [];
		const api: string[] = [];
		page.on("request", (request) => {
			if (request.resourceType() === "document") {
				documents.push(request.url());
			} else if (request.url().startsWith(seed.api_url)) {
				api.push(request.url());
			}
		});
		await tabRow(page).getByRole("link", { name: "Plots" }).click();

		await expect(page).toHaveURL(nextPath(hashbrown.slug, "plots"));
		await expect(
			page.getByRole("heading", { level: 1, name: "Plots" }),
		).toBeVisible();
		await expect(tabRow(page).locator('[aria-current="page"]')).toHaveText(
			"Plots",
		);
		await expect(page.getByRole("status")).toHaveText(
			`Plots | ${hashbrown.name} | Bencher`,
		);
		await settle(page);
		expect(documents).toEqual([]);
		expect(api).toEqual([]);
	});

	// Kills a cache that is not persisted, or is restored only after the API
	// answers, and a spinner over data already seen.
	test("a warm reload paints the names from the cache before the API answers", async ({
		page,
	}) => {
		await page.goto(nextPath(hashbrown.slug, "reports"));
		await expect(
			tabRow(page).getByRole("link", { name: "Alerts 1 active" }),
		).toBeVisible();
		await expect.poll(() => persistedQueries(page)).toBeGreaterThan(0);

		const held: Route[] = [];
		await page.route(`${seed.api_url}/**`, (route) => {
			held.push(route);
		});
		await page.reload();

		await expect(
			crumbs(page).getByRole("link", { name: seed.organization.name }),
		).toBeVisible();
		await expect(
			crumbs(page).getByRole("link", { name: hashbrown.name }),
		).toBeVisible();
		await expect(
			tabRow(page).getByRole("link", { name: "Alerts 1 active" }),
		).toBeVisible();
		await expect(page.locator('[aria-busy="true"]')).toHaveCount(0);
		expect(held.length).toBeGreaterThan(0);

		await page.unrouteAll({ behavior: "ignoreErrors" });
	});

	// Kills an app that drops the focus a reader gave the static page.
	test("a tab focused before the app loads keeps the focus", async ({
		page,
	}) => {
		let release = () => {};
		const released = new Promise<void>((resolve) => {
			release = resolve;
		});
		await page.route("**/*.js", async (route) => {
			await released;
			await route.continue();
		});
		await page.goto(nextPath(hashbrown.slug, "reports"), {
			waitUntil: "commit",
		});
		const plots = tabRow(page).getByRole("link", { name: "Plots" });
		await plots.focus();

		release();

		await expect(
			page.getByRole("heading", { level: 1, name: "Reports" }),
		).toBeVisible();
		await expect(plots).toBeFocused();
	});

	// Kills a first paint that waits for the app to name the project.
	test("a returning reader's first paint names the project before any module loads", async ({
		page,
	}) => {
		await page.goto(nextPath(hashbrown.slug, "reports"));
		await expect(
			tabRow(page).getByRole("link", { name: "Alerts 1 active" }),
		).toBeVisible();

		await page.route("**/*.js", (route) => route.abort());
		await page.reload();

		await expect(
			crumbs(page).getByRole("link", { name: seed.organization.name }),
		).toBeVisible();
		await expect(
			crumbs(page).getByRole("link", { name: hashbrown.name }),
		).toBeVisible();
		await expect(
			tabRow(page).getByRole("link", { name: "Alerts 1 active" }),
		).toBeVisible();
	});
});

/** How many queries the signed in reader has persisted. */
const persistedQueries = (page: Page) =>
	page.evaluate(
		(uuid) =>
			new Promise<number>((resolve) => {
				const open = indexedDB.open("bencher-console");
				open.onerror = () => resolve(0);
				open.onsuccess = () => {
					const db = open.result;
					if (!db.objectStoreNames.contains("cache")) {
						db.close();
						return resolve(0);
					}
					const get = db.transaction("cache").objectStore("cache").get(uuid);
					get.onsuccess = () => {
						db.close();
						resolve(get.result?.state?.queries?.length ?? 0);
					};
					get.onerror = () => {
						db.close();
						resolve(0);
					};
				};
			}),
		seed.member.user.uuid,
	);
