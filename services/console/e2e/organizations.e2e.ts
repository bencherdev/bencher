import type {
	APIRequestContext,
	BrowserContext,
	BrowserContextOptions,
	Page,
} from "@playwright/test";
import type {
	JsonAuthUser,
	JsonOrganization,
	JsonProject,
} from "../src/types/bencher";
import { createReader, unique } from "./api";
import { axeInBothThemes } from "./axe";
import { expect, seed, signedIn, test } from "./fixtures";

const ORGANIZATIONS = "/console/organizations";
const heading = (page: Page) =>
	page.getByRole("heading", { level: 1, name: "Organizations" });
const dialog = (page: Page) =>
	page.getByRole("dialog", { name: "New organization" });
const table = (page: Page) =>
	page.getByRole("table", { name: "Your organizations" });

/** The API, as `reader`. */
const send = async <T>(
	request: APIRequestContext,
	reader: JsonAuthUser,
	path: string,
	data?: unknown,
): Promise<T> => {
	const response = await request.fetch(`${seed.api_url}${path}`, {
		method: data === undefined ? "GET" : "POST",
		headers: { Authorization: `Bearer ${reader.token}` },
		...(data === undefined ? {} : { data }),
	});
	if (!response.ok()) {
		throw new Error(`${path} answered ${response.status()}`);
	}
	return (await response.json()) as T;
};

/** A page of its own for a new reader, whom each test may change. */
const readerPage = async (
	newContext: (options?: BrowserContextOptions) => Promise<BrowserContext>,
	reader: JsonAuthUser,
) => (await newContext({ storageState: signedIn(reader) })).newPage();

test.describe("Organizations", () => {
	// Kills a dialog that leaves without making the organization, lands
	// anywhere but its New project, or leaves the bootstrap the page and the
	// switcher read without it, and a New project that leaves the bootstrap
	// without the project it made.
	test("New organization makes one and lands on its New project, which makes its first project", async ({
		newContext,
		request,
	}) => {
		const reader = await createReader(request);
		const page = await readerPage(newContext, reader);
		await page.goto(ORGANIZATIONS);
		// Signing up made the reader an organization of their own.
		await expect(
			page.getByText("1 organization", { exact: true }),
		).toBeVisible();
		const name = unique("Bagge Farm");

		await page.getByRole("button", { name: "New organization" }).click();
		await dialog(page).getByRole("textbox", { name: "Name" }).fill(name);
		await dialog(page)
			.getByRole("button", { name: "Create organization" })
			.click();

		await expect(page).toHaveURL(
			/\/console\/organizations\/([a-z0-9-]+)\/projects\/add$/,
		);
		const slug = new URL(page.url()).pathname.split("/")[3] ?? "";
		const made = await send<JsonOrganization>(
			request,
			reader,
			`/v0/organizations/${slug}`,
		);
		expect(made.name).toBe(name);
		await expect(
			page.getByRole("combobox", { name: "Organization" }),
		).toHaveValue(made.uuid);

		await page.getByRole("textbox", { name: "Name" }).fill(unique("Rosti"));
		await page.getByRole("button", { name: "Create project" }).click();
		await expect(page).toHaveURL(/\/console\/projects\/[a-z0-9-]+\/$/);

		// The bootstrap never answers, so the row can come only from the cache.
		await page.route(`${seed.api_url}/v0/console/users/**`, () => {});
		await page.goto(ORGANIZATIONS);
		await expect(
			table(page).getByRole("row", { name: new RegExp(name) }),
		).toContainText("1 project");
	});

	// Kills a list that drops an organization, counts projects across
	// organizations, offers New project where projects are, or forgets the
	// list across a reload.
	test("lists the reader's organizations with their projects, after a reload too", async ({
		newContext,
		request,
	}) => {
		const reader = await createReader(request);
		const empty = await send<JsonOrganization>(
			request,
			reader,
			"/v0/organizations",
			{ name: unique("Bagge Farm") },
		);
		const full = await send<JsonOrganization>(
			request,
			reader,
			"/v0/organizations",
			{ name: unique("Nowhere Labs") },
		);
		await send<JsonProject>(
			request,
			reader,
			`/v0/organizations/${full.slug}/projects`,
			{ name: unique("Rosti"), visibility: "public" },
		);
		const page = await readerPage(newContext, reader);

		for (const load of [() => page.goto(ORGANIZATIONS), () => page.reload()]) {
			await load();
			await expect(page.getByText("3 organizations")).toBeVisible();
			const emptyRow = table(page).getByRole("row", { name: empty.name });
			const fullRow = table(page).getByRole("row", { name: full.name });
			await expect(emptyRow).toContainText("No projects");
			await expect(
				emptyRow.getByRole("link", { name: "New project" }),
			).toHaveAttribute(
				"href",
				`/console/organizations/${empty.slug}/projects/add`,
			);
			await expect(fullRow).toContainText("1 project");
			await expect(
				fullRow.getByRole("link", { name: "New project" }),
			).toHaveCount(0);
			await expect(
				fullRow.getByRole("link", { name: full.name }),
			).toHaveAttribute("href", `/console/organizations/${full.slug}/settings`);
		}
	});
});

test.describe("an organization's row", () => {
	test.use({ storageState: signedIn(seed.member) });

	// Kills a row that leads to another organization's settings.
	test("opens the organization's General", async ({ page }) => {
		await page.goto(ORGANIZATIONS);
		await table(page)
			.getByRole("link", { name: seed.organization.name })
			.click();

		await expect(page).toHaveURL(
			`${ORGANIZATIONS}/${seed.organization.slug}/settings`,
		);
		await expect(page.getByRole("textbox", { name: "Name" })).toHaveValue(
			seed.organization.name,
		);
	});
});

test.describe("from Profile", () => {
	test.use({ storageState: signedIn(seed.member) });

	// Kills a rail that leaves the console's app for Organizations, reloading
	// the page.
	test("the rail opens Organizations without a reload", async ({ page }) => {
		await page.goto(`/console/users/${seed.member.user.slug}/settings`);
		await expect(page.getByRole("textbox", { name: "Name" })).toHaveValue(
			seed.member.user.name,
		);
		await page.evaluate(() => {
			(window as Window & { stayed?: boolean }).stayed = true;
		});

		await page
			.getByRole("navigation", { name: "Account", exact: true })
			.getByRole("link", { name: "Organizations" })
			.click();

		await expect(page).toHaveURL(ORGANIZATIONS);
		await expect(
			table(page).getByRole("link", { name: seed.organization.name }),
		).toBeVisible();
		await expect(page).toHaveTitle("Organizations | Bencher");
		expect(
			await page.evaluate(
				() => (window as Window & { stayed?: boolean }).stayed,
			),
		).toBe(true);
	});
});

test.describe("New organization's own path", () => {
	test.use({ storageState: signedIn(seed.member) });

	// Kills the classic form's path left with no page, and a closed dialog that
	// leaves the reader on it, where a reload opens it again.
	test("opens the dialog, and closing it goes back to the list", async ({
		page,
	}) => {
		await page.goto(`${ORGANIZATIONS}/add`);

		await expect(dialog(page)).toBeVisible();
		await page.keyboard.press("Escape");

		await expect(dialog(page)).toHaveCount(0);
		await expect(page).toHaveURL(ORGANIZATIONS);
		await expect(heading(page)).toBeVisible();
	});
});

test.describe("accessibility", () => {
	test.use({ storageState: signedIn(seed.member) });

	// Kills contrast, names, and roles that fail axe in either theme.
	test("Organizations passes axe in both themes", async ({ page }) => {
		await axeInBothThemes(page, ORGANIZATIONS, () =>
			expect(
				table(page).getByRole("link", { name: seed.organization.name }),
			).toBeVisible(),
		);
	});
});
