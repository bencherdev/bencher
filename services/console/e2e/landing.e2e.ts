import type { Page } from "@playwright/test";
import {
	VERSION_KEY,
	expect,
	consolePath,
	seed,
	signedIn,
	signedOut,
	tabRow,
	test,
} from "./fixtures";

const { hashbrown, version_zero } = seed.projects;
const HERO = "Catch performance regressions before they merge";
const WATCH = "landing:root";

const remembered = (versions: Record<string, number>) =>
	signedIn(seed.member, { [VERSION_KEY]: JSON.stringify(versions) });

interface RootRendering {
	/** The root page ran a frame. */
	frame?: true;
	/** The browser reported a paint of the root page. */
	paint?: true;
	/** A frame had the marketing page on screen. */
	marketing?: true;
}

/** Notes what the root page renders; the note outlives the root page. */
const watchRootRendering = (page: Page) =>
	page.addInitScript(
		([key]) => {
			if (location.pathname !== "/") {
				return;
			}
			const note = (seen: string) => {
				const notes = JSON.parse(sessionStorage.getItem(key) ?? "{}");
				notes[seen] = true;
				sessionStorage.setItem(key, JSON.stringify(notes));
			};
			new PerformanceObserver((list) => {
				if (list.getEntries().length > 0) {
					note("paint");
				}
			}).observe({ type: "paint", buffered: true });
			const frame = () => {
				note("frame");
				if (document.querySelector(".hero-landing")?.checkVisibility()) {
					note("marketing");
				}
				requestAnimationFrame(frame);
			};
			requestAnimationFrame(frame);
		},
		[WATCH] as const,
	);

const rootRendering = async (page: Page): Promise<RootRendering> =>
	JSON.parse(
		(await page.evaluate((key) => sessionStorage.getItem(key), WATCH)) ?? "{}",
	);

test.describe("a signed in reader with a remembered version 1 project", () => {
	test.use({ storageState: remembered({ [hashbrown.slug]: 1 }) });

	// Kills a root page that renders at all, asks the API, or lets its own
	// redirect overtake the landing while the project page is slow to answer,
	// and a landing on anything but Runs.
	test("lands on Runs at the project's root with the root page never rendering or asking the API", async ({
		page,
	}) => {
		await watchRootRendering(page);
		const order: string[] = [];
		page.on("request", (request) => {
			if (request.resourceType() === "document") {
				order.push(new URL(request.url()).pathname);
			} else if (request.url().startsWith(seed.api_url)) {
				order.push("api");
			}
		});
		await page.route(
			(url) => url.pathname.startsWith(consolePath(hashbrown.slug)),
			async (route) => {
				if (route.request().resourceType() === "document") {
					await new Promise((resolve) => setTimeout(resolve, 1_000));
				}
				await route.continue();
			},
		);

		await page.goto("/");

		await expect(page).toHaveURL(consolePath(hashbrown.slug));
		await expect(
			tabRow(page).getByRole("link", { name: "Alerts 1 active" }),
		).toBeVisible();
		await expect(
			page.getByRole("table", { name: "Runs, newest first" }),
		).toBeVisible();
		expect(order.slice(0, 2)).toEqual(["/", consolePath(hashbrown.slug)]);
		expect(await rootRendering(page)).toEqual({});
	});
});

test.describe("a signed in reader with a remembered version 0 project", () => {
	test.use({ storageState: remembered({ [version_zero.slug]: 0 }) });

	// Kills a version 0 project sent anywhere but Runs at its root in the console.
	test("lands on Runs at the project's root", async ({ page }) => {
		await page.goto("/");
		await expect(page).toHaveURL(consolePath(version_zero.slug));
		await expect(
			page.getByRole("heading", { level: 1, name: "6 runs" }),
		).toBeVisible();
	});
});

test.describe("a reader who is not signed in", () => {
	test.use({
		storageState: {
			...signedOut,
			origins: [
				{
					origin: seed.console_url,
					localStorage: [
						{
							name: VERSION_KEY,
							value: JSON.stringify({ [hashbrown.slug]: 1 }),
						},
					],
				},
			],
		},
	});

	// Kills a root page that sends a reader who is not signed in to a project,
	// and a watch that cannot see the root page render.
	test("sees the marketing page", async ({ page }) => {
		await watchRootRendering(page);
		await page.goto("/");
		await expect(
			page.getByRole("heading", { level: 1, name: HERO }),
		).toBeVisible();
		await expect
			.poll(() => rootRendering(page))
			.toEqual({ frame: true, paint: true, marketing: true });
		await expect(page).toHaveURL("/");
	});
});
