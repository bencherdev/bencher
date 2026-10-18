import type { Page } from "@playwright/test";
import type { JsonOrganization } from "../src/types/bencher";
import {
	createOrganization,
	createProject,
	readOrganization,
	unique,
} from "./api";
import { axeInBothThemes } from "./axe";
import {
	VERSION_KEY,
	crumb,
	expect,
	nextPath,
	seed,
	signedIn,
	test,
} from "./fixtures";

const { hashbrown, version_zero } = seed.projects;

const generalPath = (organization: string) =>
	`/next/console/organizations/${organization}/settings`;
const classicGeneral = (organization: string) =>
	`/console/organizations/${organization}/settings`;
const heading = (page: Page) =>
	page.getByRole("heading", { level: 1, name: "General" });
const textbox = (page: Page, name: string) =>
	page.getByRole("textbox", { name });
const save = (page: Page) => page.getByRole("button", { name: "Save changes" });

test.use({ storageState: signedIn(seed.member) });

test.describe("General", () => {
	// Kills a rename that never reaches the API, and one the page forgets on a reload.
	test("a new name reaches the API and survives a reload", async ({
		page,
		request,
	}) => {
		const organization = await createOrganization(request);
		await page.goto(generalPath(organization.slug));
		const renamed = unique("Bagge Ranch");

		await textbox(page, "Name").fill(renamed);
		await save(page).click();

		await expect(page.getByRole("main").getByRole("status")).toHaveText(
			"Saved",
		);
		const read = (await (
			await readOrganization(request, organization.slug)
		).json()) as JsonOrganization;
		expect(read.name).toBe(renamed);
		await page.reload();
		await expect(textbox(page, "Name")).toHaveValue(renamed);
		await expect(crumb(page, renamed)).toBeVisible();
	});

	// Kills a slug change sent without the reader's confirmation, a page left
	// at a slug that no longer answers, and a move the reload undoes.
	test("a slug change confirms, then moves the page", async ({
		page,
		request,
	}) => {
		const organization = await createOrganization(request);
		await page.goto(generalPath(organization.slug));
		const slug = `${organization.slug}-x`;

		await textbox(page, "Slug").fill(slug);
		await save(page).click();
		const dialog = page.getByRole("alertdialog", {
			name: `Change the slug to ${slug}?`,
		});
		await expect(dialog).toContainText(
			"Every existing link to this organization breaks.",
		);
		expect((await readOrganization(request, slug)).status()).toBe(404);

		await dialog.getByRole("button", { name: "Change slug" }).click();

		await expect(page).toHaveURL(generalPath(slug));
		expect((await readOrganization(request, slug)).status()).toBe(200);
		await page.reload();
		await expect(textbox(page, "Slug")).toHaveValue(slug);
	});

	// Kills a confirmation that names none of the organization's projects, a
	// delete that never reaches the API, a reader left on the deleted
	// organization, and a deleted project the browser keeps as a landing.
	test("deleting an organization names its projects and lands on Organizations", async ({
		newContext,
		request,
	}) => {
		const organization = await createOrganization(request);
		const project = await createProject(request, {
			organization: organization.slug,
		});
		const page = await (
			await newContext({
				storageState: signedIn(seed.member, {
					[VERSION_KEY]: JSON.stringify({
						[hashbrown.slug]: 1,
						[project.slug]: 1,
					}),
				}),
			})
		).newPage();
		await page.goto(generalPath(organization.slug));

		await page
			.getByRole("region", { name: "Danger" })
			.getByRole("button", { name: "Delete organization" })
			.click();
		const dialog = page.getByRole("alertdialog", {
			name: `Delete ${organization.slug}?`,
		});
		await expect(dialog).toContainText(`${project.name} goes with it.`);
		const confirm = dialog.getByRole("button", { name: "Delete organization" });
		await expect(confirm).toBeDisabled();
		await dialog
			.getByRole("textbox", { name: `Type ${organization.slug} to confirm` })
			.fill(organization.slug);
		await confirm.click();

		await expect(page).toHaveURL("/next/console/organizations");
		expect((await readOrganization(request, organization.slug)).status()).toBe(
			404,
		);
		const versions = JSON.parse(
			(await page.evaluate((key) => localStorage.getItem(key), VERSION_KEY)) ??
				"{}",
		);
		expect(versions).toEqual({ [hashbrown.slug]: 1 });
	});

	// Kills the classic shortcuts to an organization lost under /next/.
	test("the organization's root and its projects open General", async ({
		page,
	}) => {
		for (const path of ["", "/projects"]) {
			await page.goto(
				`/next/console/organizations/${seed.organization.slug}${path}`,
			);
			await expect(page).toHaveURL(generalPath(seed.organization.slug));
			await expect(heading(page)).toBeVisible();
		}
	});
});

test.describe("from a project page", () => {
	// Kills a switcher footer that still leads to the classic settings, or to
	// another organization's, and a page titled for another section.
	test("the switcher's footer opens the organization's General", async ({
		page,
	}) => {
		await page.goto(nextPath(hashbrown.slug, "runs"));
		await crumb(page, `${seed.organization.name} / ${hashbrown.name}`).click();
		await page
			.getByRole("dialog", { name: "Switch project" })
			.getByRole("link", { name: `${seed.organization.name} settings` })
			.click();

		await expect(page).toHaveURL(generalPath(seed.organization.uuid));
		await expect(textbox(page, "Name")).toHaveValue(seed.organization.name);
		await expect(crumb(page, seed.organization.name)).toBeVisible();
		await expect(page).toHaveTitle(
			`General | ${seed.organization.name} | Bencher`,
		);
	});
});

const remembered = (versions: Record<string, number>) =>
	signedIn(seed.member, { [VERSION_KEY]: JSON.stringify(versions) });

test.describe("the classic settings, for a reader with a version 1 project", () => {
	test.use({ storageState: remembered({ [hashbrown.slug]: 1 }) });

	// Kills a classic organization page that keeps a reader the new one serves.
	test("sends the reader to General under /next/", async ({ page }) => {
		await page.goto(classicGeneral(seed.organization.slug));

		await expect(page).toHaveURL(generalPath(seed.organization.slug));
		await expect(heading(page)).toBeVisible();
	});
});

test.describe("the classic settings, for a reader of version 0 projects only", () => {
	test.use({ storageState: remembered({ [version_zero.slug]: 0 }) });

	// Kills a classic organization page that sends away a reader the new one does not serve.
	test("keeps the reader", async ({ page }) => {
		await page.goto(classicGeneral(seed.organization.slug));
		await page.waitForLoadState("networkidle");

		await expect(page).toHaveURL(classicGeneral(seed.organization.slug));
	});
});

test.describe("accessibility", () => {
	const path = generalPath(seed.organization.slug);
	const loaded = (page: Page) => () =>
		expect(textbox(page, "Name")).toHaveValue(seed.organization.name);

	// Kills contrast, names, and roles that fail axe in either theme.
	test("General passes axe in both themes", async ({ page }) => {
		await axeInBothThemes(page, path, loaded(page));
	});

	test("the slug confirmation passes axe in both themes", async ({ page }) => {
		await axeInBothThemes(
			page,
			path,
			async () => {},
			async () => {
				await loaded(page)();
				await textbox(page, "Slug").fill("pompeii-axe");
				await save(page).click();
				await expect(page.getByRole("alertdialog")).toBeVisible();
			},
		);
	});

	test("the delete confirmation passes axe in both themes", async ({
		page,
	}) => {
		await axeInBothThemes(
			page,
			path,
			async () => {},
			async () => {
				await page.getByRole("button", { name: "Delete organization" }).click();
				await expect(page.getByRole("alertdialog")).toBeVisible();
			},
		);
	});
});
