import type { Page } from "@playwright/test";
import type { JsonOrganization, JsonProject } from "../src/types/bencher";
import { createReader, unique } from "./api";
import { axeInBothThemes } from "./axe";
import { crumb, expect, seed, signedIn, test } from "./fixtures";

const newProject = (organization: string) =>
	`/console/organizations/${organization}/projects/add`;
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
		await page.goto(`/console/organizations/${organization.slug}/settings`);
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

		// The API still makes version 0 projects, which open in the console.
		await expect(page).toHaveURL(/\/console\/projects\/([a-z0-9-]+)\/$/);
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

test.describe("New project as a member", () => {
	test.use({ storageState: signedIn(seed.member) });

	// Kills a slug line that grows when the slug rule's code arrives after the
	// reader typed, which moves every field below it.
	test("the slug arriving late moves nothing", async ({ page }) => {
		let release = () => {};
		const held = new Promise<void>((resolve) => {
			release = resolve;
		});
		await page.route("**/*.wasm", async (route) => {
			await held;
			await route.continue();
		});
		await page.goto(newProject(seed.organization.slug));
		const url = textbox(page, "URL");
		await expect(url).toBeVisible();
		await textbox(page, "Name").fill("Hash Brown");
		const before = await url.boundingBox();
		release();
		await expect(page.getByText("hash-brown", { exact: true })).toBeVisible();
		expect(await url.boundingBox()).toEqual(before);
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
