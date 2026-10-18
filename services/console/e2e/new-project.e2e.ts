import type { Page } from "@playwright/test";
import type { JsonOrganization, JsonProject } from "../src/types/bencher";
import { createReader, unique } from "./api";
import { axeInBothThemes } from "./axe";
import { VERSION_KEY, crumb, expect, seed, signedIn, test } from "./fixtures";

const { hashbrown, version_zero } = seed.projects;

const newProject = (organization: string) =>
	`/next/console/organizations/${organization}/projects/add`;
const textbox = (page: Page, name: string) =>
	page.getByRole("textbox", { name });
const radio = (page: Page, name: string) => page.getByRole("radio", { name });

test.describe("New project", () => {
	// Kills a footer that leads anywhere but this page, a page that cannot
	// read its organization by UUID, Private offered without a plan, and a
	// create that lands anywhere but where the API's version lives.
	test("opens from the switcher's footer and lands where the project's version lives", async ({
		newContext,
		request,
	}) => {
		const reader = await createReader(request);
		const own = await request.fetch(`${seed.api_url}/v0/organizations`, {
			headers: { Authorization: `Bearer ${reader.token}` },
		});
		const [organization] = (await own.json()) as JsonOrganization[];
		if (!organization) {
			throw new Error("Signing up made no organization");
		}
		const page = await (
			await newContext({ storageState: signedIn(reader) })
		).newPage();
		await page.goto(
			`/next/console/organizations/${organization.slug}/settings`,
		);
		await expect(textbox(page, "Name")).toHaveValue(organization.name);

		await crumb(page, organization.name).click();
		await page
			.getByRole("dialog", { name: "Switch project" })
			.getByRole("link", { name: "New project" })
			.click();

		await expect(page).toHaveURL(newProject(organization.uuid));
		await expect(page).toHaveTitle(
			`New project | ${organization.name} | Bencher`,
		);
		await expect(
			page.getByRole("combobox", { name: "Organization" }),
		).toHaveValue(organization.uuid);
		await expect(radio(page, "Public")).toBeChecked();
		await expect(radio(page, "Private")).toBeDisabled();
		const name = unique("Hash Brown");
		await textbox(page, "Name").fill(name);
		await page.getByRole("button", { name: "Create project" }).click();

		// The API still makes version 0 projects, which the classic console serves.
		await expect(page).toHaveURL(/\/console\/projects\/([a-z0-9-]+)\/plots$/);
		const slug = new URL(page.url()).pathname.split("/")[3] ?? "";
		const project = (await (
			await request.fetch(`${seed.api_url}/v0/projects/${slug}`, {
				headers: { Authorization: `Bearer ${reader.token}` },
			})
		).json()) as JsonProject;
		expect(project).toMatchObject({
			name,
			organization: organization.uuid,
			visibility: "public",
			bmf_version: 0,
		});
	});
});

test.describe("New project for an organization with a license", () => {
	test.use({ storageState: signedIn(seed.member) });

	// Kills Private held back from an organization whose license allows it,
	// and contrast, names, and roles that fail axe in either theme.
	test("offers Private and passes axe in both themes", async ({ page }) => {
		await axeInBothThemes(
			page,
			newProject(seed.organization.slug),
			async () => {
				await expect(radio(page, "Private")).toBeEnabled();
				await textbox(page, "Name").fill("Hash Brown");
				await expect(
					page.getByText("hash-brown", { exact: true }),
				).toBeVisible();
			},
		);
	});
});

const remembered = (versions: Record<string, number>) =>
	signedIn(seed.member, { [VERSION_KEY]: JSON.stringify(versions) });

test.describe("the classic New project, for a reader with a version 1 project", () => {
	test.use({ storageState: remembered({ [hashbrown.slug]: 1 }) });

	// Kills a classic form that keeps a reader the new page serves.
	test("sends the reader to New project under /next/", async ({ page }) => {
		await page.goto(
			`/console/organizations/${seed.organization.slug}/projects/add`,
		);
		await expect(page).toHaveURL(newProject(seed.organization.slug));
		await expect(
			page.getByRole("heading", { level: 1, name: "New project" }),
		).toBeVisible();
	});
});

test.describe("the classic New project, for a reader of version 0 projects only", () => {
	test.use({ storageState: remembered({ [version_zero.slug]: 0 }) });

	// Kills a classic form that sends away a reader the new one does not serve.
	test("keeps the reader", async ({ page }) => {
		const classic = `/console/organizations/${seed.organization.slug}/projects/add`;
		await page.goto(classic);
		await page.waitForLoadState("networkidle");

		expect(new URL(page.url()).pathname).toBe(classic);
	});
});
