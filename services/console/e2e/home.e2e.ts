import type { Page } from "@playwright/test";
import type { JsonAuthUser } from "../src/types/bencher";
import { createOwnProject, createProject, createReader } from "./api";
import {
	VERSION_KEY,
	expect,
	nextPath,
	seed,
	signedIn,
	test,
} from "./fixtures";

const { hashbrown } = seed.projects;
const ORGANIZATIONS = "/next/console/organizations";

const keysPath = (reader: JsonAuthUser) =>
	`/next/console/users/${reader.user.slug}/keys`;
const wordmark = (page: Page) =>
	page.getByRole("banner").getByRole("link", { name: "Bencher, home" });

test.describe("the wordmark", () => {
	test.use({
		storageState: signedIn(seed.member, {
			[VERSION_KEY]: JSON.stringify({ [hashbrown.slug]: 1 }),
		}),
	});

	// Kills a wordmark that leads to the classic console, or anywhere but the
	// main project the browser remembers.
	test("opens the main project the browser remembers, in the app", async ({
		page,
	}) => {
		await page.goto(keysPath(seed.member));
		await expect(
			page.getByRole("heading", { level: 1, name: "Keys" }),
		).toBeVisible();
		await page.evaluate(() => {
			(window as unknown as { stayed: boolean }).stayed = true;
		});
		await wordmark(page).click();

		await expect(page).toHaveURL(nextPath(hashbrown.slug));
		expect(
			await page.evaluate(
				() => (window as unknown as { stayed?: boolean }).stayed,
			),
		).toBe(true);
	});
});

// Kills a wordmark whose link keeps the project the shell first painted
// with, rather than the one the reader moved to since.
test("names the project the reader last opened", async ({
	newContext,
	request,
}) => {
	const project = await createProject(request);
	const page = await (
		await newContext({
			storageState: signedIn(seed.member, {
				[VERSION_KEY]: JSON.stringify({ [hashbrown.slug]: 1 }),
			}),
		})
	).newPage();
	await page.goto(nextPath(project.slug, "settings"));
	await expect
		.poll(() => page.evaluate((key) => localStorage.getItem(key), VERSION_KEY))
		.toContain(project.slug);
	await wordmark(page).hover();

	await expect(wordmark(page)).toHaveAttribute("href", nextPath(project.slug));
});

// Kills a reader with nothing remembered sent to a project, or to the classic console.
test("opens Organizations for a reader with nothing remembered", async ({
	newContext,
}) => {
	const page = await (
		await newContext({ storageState: signedIn(seed.member) })
	).newPage();
	await page.goto(keysPath(seed.member));
	await wordmark(page).click();

	await expect(page).toHaveURL(ORGANIZATIONS);
});

// Kills a delete of a reader's last project that lands on a project page.
test("deleting the reader's last project lands on Organizations", async ({
	newContext,
	request,
}) => {
	const reader = await createReader(request);
	const project = await createOwnProject(request, reader);
	const page = await (
		await newContext({ storageState: signedIn(reader) })
	).newPage();
	await page.goto(nextPath(project.slug, "settings"));
	await page.getByRole("button", { name: "Delete project" }).click();
	const dialog = page.getByRole("alertdialog", {
		name: `Delete ${project.slug}?`,
	});
	await dialog
		.getByRole("textbox", { name: `Type ${project.slug} to confirm` })
		.fill(project.slug);
	await dialog.getByRole("button", { name: "Delete project" }).click();

	await expect(page).toHaveURL(ORGANIZATIONS);
});
