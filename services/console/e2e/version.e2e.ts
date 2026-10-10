import type { Page } from "@playwright/test";
import {
	VERSION_KEY,
	expect,
	nextPath,
	seed,
	settle,
	signedIn,
	test,
} from "./fixtures";

const { hashbrown, version_zero } = seed.projects;

// The classic Reports page adds its own paging to the query.
const classicReports = (url: URL) =>
	url.pathname === `/console/projects/${version_zero.slug}/reports`;

const remembered = (versions: Record<string, number>) =>
	signedIn(seed.member, { [VERSION_KEY]: JSON.stringify(versions) });

// The page can move between consoles, so a read can land mid navigation.
const rememberedVersion = (page: Page, slug: string) =>
	page
		.evaluate(
			([key, slug]) => JSON.parse(localStorage.getItem(key) ?? "{}")[slug],
			[VERSION_KEY, slug] as const,
		)
		.catch(() => undefined);

test.describe("with no version memory", () => {
	test.use({ storageState: signedIn(seed.member) });

	// Kills a new console that keeps a version 0 project.
	test("a version 0 project under /next/ lands on its classic page", async ({
		page,
	}) => {
		await page.goto(nextPath(version_zero.slug, "reports"));
		await expect(page).toHaveURL(classicReports);
	});

	// Kills a classic console that keeps a version 1 project.
	test("a classic page of a version 1 project lands on /next/", async ({
		page,
	}) => {
		await page.goto(`/console/projects/${hashbrown.slug}/reports`);
		await expect(page).toHaveURL(nextPath(hashbrown.slug, "reports"));
	});
});

test.describe("with the version memory", () => {
	test.use({ storageState: remembered({ [hashbrown.slug]: 1 }) });

	// Kills a classic redirect that waits for the project to load.
	test("a classic page redirects before it asks for the project", async ({
		page,
	}) => {
		const order: string[] = [];
		page.on("request", (request) => {
			const url = new URL(request.url());
			if (request.resourceType() === "document") {
				order.push(url.pathname);
			} else if (
				request.url() === `${seed.api_url}/v0/projects/${hashbrown.slug}`
			) {
				order.push("project");
			}
		});

		await page.goto(`/console/projects/${hashbrown.slug}/reports`);
		await expect(page).toHaveURL(nextPath(hashbrown.slug, "reports"));
		await expect(
			page.getByRole("heading", { level: 1, name: "Reports" }),
		).toBeVisible();

		expect(order.slice(0, 2)).toEqual([
			`/console/projects/${hashbrown.slug}/reports`,
			nextPath(hashbrown.slug, "reports"),
		]);
	});
});

test.describe("with a stale version 1 memory", () => {
	test.use({ storageState: remembered({ [version_zero.slug]: 1 }) });

	// Kills a memory that only the inline script reads and nothing corrects.
	test("the project corrects it and the reader stays on the classic page", async ({
		page,
	}) => {
		await page.goto(`/console/projects/${version_zero.slug}/reports`);

		await expect.poll(() => rememberedVersion(page, version_zero.slug)).toBe(0);
		await expect(page).toHaveURL(classicReports);
	});
});

test.describe("with a stale version 0 memory", () => {
	test.use({ storageState: remembered({ [hashbrown.slug]: 0 }) });

	// Kills a classic console that sends a version 1 project to /next/ without
	// correcting the memory.
	test("the project corrects it and the reader ends on /next/", async ({
		page,
	}) => {
		await page.goto(nextPath(hashbrown.slug, "reports"));

		await expect.poll(() => rememberedVersion(page, hashbrown.slug)).toBe(1);
		await expect(page).toHaveURL(nextPath(hashbrown.slug, "reports"));
	});
});

test.describe("with a saved response from before the move to version 1", () => {
	test.use({ storageState: signedIn(seed.member) });

	// Kills a new console that switches on a response restored from the cache.
	test("the reader settles on /next/", async ({ page }) => {
		await page.goto(nextPath(hashbrown.slug, "reports"));
		await expect
			.poll(async () =>
				(await cache(page))?.state.queries.some(
					(query) => query.queryHash === BOOTSTRAP_HASH,
				),
			)
			.toBe(true);
		// Off the new console, nothing saves over the entry rewritten below.
		await page.goto("/help");
		const entry = await cache(page);
		const bootstrap = entry?.state.queries.find(
			(query) => query.queryHash === BOOTSTRAP_HASH,
		);
		if (!entry || !bootstrap) {
			throw new Error("The new console saved no response for the project");
		}
		const saved = bootstrap.state.data as { project: { bmf_version: number } };
		saved.project.bmf_version = 0;
		await cache(page, entry);

		// The saved response is restored well before the fresh one arrives.
		const bootstrapUrl = `${seed.api_url}/v0/projects/${hashbrown.slug}/console`;
		await page.route(bootstrapUrl, async (route) => {
			await new Promise((resolve) => setTimeout(resolve, 500));
			await route.continue();
		});
		const visited: string[] = [];
		page.on("request", (request) => {
			if (request.resourceType() === "document") {
				visited.push(new URL(request.url()).pathname);
			}
		});
		const fresh = page.waitForResponse(bootstrapUrl, { timeout: 10_000 });
		// A console that switches cuts this navigation short.
		await page.goto(nextPath(hashbrown.slug, "reports")).catch(() => {});
		await fresh;
		await settle(page);

		expect(visited).toEqual([nextPath(hashbrown.slug, "reports")]);
		expect(await rememberedVersion(page, hashbrown.slug)).toBe(1);
	});
});

type CacheEntry = {
	state: {
		queries: { queryHash: string; state: { data: unknown } }[];
	};
};

const BOOTSTRAP_HASH = JSON.stringify(["console", "project", hashbrown.slug]);

/** The reader's saved cache, read or replaced as the new console stores it. */
const cache = (page: Page, entry?: CacheEntry) =>
	page.evaluate(
		([reader, entry]) =>
			new Promise<CacheEntry | undefined>((resolve, reject) => {
				const open = indexedDB.open("bencher-console", 1);
				open.onupgradeneeded = () => open.result.createObjectStore("cache");
				open.onerror = () => reject(open.error);
				open.onsuccess = () => {
					const db = open.result;
					const transaction = db.transaction(
						"cache",
						entry ? "readwrite" : "readonly",
					);
					const store = transaction.objectStore("cache");
					const request = entry ? store.put(entry, reader) : store.get(reader);
					transaction.oncomplete = () => {
						db.close();
						resolve(entry ? undefined : request.result);
					};
				};
			}),
		[seed.member.user.uuid, entry] as const,
	);
