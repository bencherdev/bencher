import type { Page } from "@playwright/test";
import { createReader } from "./api";
import {
	READER_KEY,
	consolePath,
	ended,
	expect,
	logInTo,
	persistedQueries,
	seed,
	signedIn,
	signedOut,
	test,
	RELAY,
} from "./fixtures";
import { inSession } from "./session";

const hint = (page: Page) =>
	page.evaluate((key) => localStorage.getItem(key), READER_KEY);

test.describe("signed out", () => {
	test.use({ storageState: signedOut });

	// Kills a console that draws a private project for nobody, and a sign in
	// that forgets where the reader was going.
	test("the private project sends the reader to sign in with the way back", async ({
		page,
	}) => {
		const path = consolePath(seed.projects.private.slug, "runs");
		await page.goto(path);
		await expect(page).toHaveURL(logInTo(path));
	});
});

test.describe("signed in", () => {
	test.use({ storageState: signedIn(seed.member) });

	// Kills a client that treats a 401 as an empty project, and a reader left
	// signed in after one.
	test("a 401 sends the reader to sign in with the way back", async ({
		page,
	}) => {
		await page.route(`${RELAY}/v0/console/projects/**`, (route) =>
			route.fulfill({ status: 401, json: { message: "Unauthorized" } }),
		);
		const path = consolePath(seed.projects.hashbrown.slug, "alerts");
		await page.goto(path);
		await expect(page).toHaveURL(logInTo(path));
		expect(await hint(page)).toBeNull();
	});
});

// Kills a console that keeps the hint or the cache of a session the API has
// ended, or forgets where the reader was going.
test("a session that ended clears the hint and the cache and asks for a sign in with the way back", async ({
	newContext,
}) => {
	const reader = await createReader();
	const page = await (
		await newContext({ storageState: signedIn(reader) })
	).newPage();
	await page.goto(`/console/users/${reader.user.slug}/keys`);
	await expect(
		page.getByRole("heading", { level: 1, name: "Keys" }),
	).toBeVisible();
	await expect
		.poll(() => persistedQueries(page, reader.user.uuid))
		.toBeGreaterThan(0);
	expect(
		(await inSession(reader, "DELETE", "/v0/auth/session")).status,
	).toBeLessThan(300);

	// A page the cache does not hold, so the console asks the API.
	const path = `/console/users/${reader.user.slug}/settings`;
	await page.goto(path);

	await expect(page).toHaveURL(logInTo(path));
	expect(await hint(page)).toBeNull();
	await expect.poll(() => persistedQueries(page, reader.user.uuid)).toBe(0);
});

test.describe("signed in to a session that ended", () => {
	test.use({ storageState: signedIn(ended(seed.member)) });

	// Kills a classic page that reads a 401 as an empty page instead of a
	// reader who must sign in again, or keeps the hint.
	test("a classic page sends the reader to sign in with the way back", async ({
		page,
	}) => {
		const path = "/console/onboard/project";
		await page.goto(path);
		await expect(page).toHaveURL(logInTo(path));
		expect(await hint(page)).toBeNull();
	});
});
