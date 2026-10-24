import type { Page, Request } from "@playwright/test";
import {
	VERSION_KEY,
	consolePath,
	expect,
	seed,
	signedIn,
	test,
} from "./fixtures";

const { hashbrown, version_zero } = seed.projects;

// The page can move, so a read can land mid navigation.
const rememberedVersion = (page: Page, slug: string) =>
	page
		.evaluate(
			([key, slug]) => JSON.parse(localStorage.getItem(key) ?? "{}")[slug],
			[VERSION_KEY, slug] as const,
		)
		.catch(() => undefined);

// Each project remembered at the other version, which nothing reads any more.
test.use({
	storageState: signedIn(seed.member, {
		[VERSION_KEY]: JSON.stringify({
			[version_zero.slug]: 1,
			[hashbrown.slug]: 0,
		}),
	}),
});

// Kills a version 0 project sent anywhere but the console, and a memory left
// at another version than the project's own.
test("a version 0 project opens in the console", async ({ page }) => {
	await page.goto(consolePath(version_zero.slug, "runs"));
	await expect(
		page.getByRole("heading", { level: 1, name: "6 runs" }),
	).toBeVisible();
	await expect(page).toHaveURL(consolePath(version_zero.slug, "runs"));
	await expect.poll(() => rememberedVersion(page, version_zero.slug)).toBe(0);
});

// Kills a classic page left in place, or one that loads a document of its
// own before it moves, for a project at either version.
test("a classic page of a project at either version lands in the console in one redirect", async ({
	page,
}) => {
	for (const project of [version_zero, hashbrown]) {
		const classic = `/console/projects/${project.slug}/reports`;
		const documents: string[] = [];
		const record = (request: Request) => {
			if (request.resourceType() === "document") {
				documents.push(new URL(request.url()).pathname);
			}
		};
		page.on("request", record);
		await page.goto(classic);
		page.off("request", record);

		await expect(page).toHaveURL(consolePath(project.slug, "runs"));
		expect(documents).toEqual([classic, consolePath(project.slug, "runs")]);
	}
});
