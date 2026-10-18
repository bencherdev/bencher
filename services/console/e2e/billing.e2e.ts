import type { Page } from "@playwright/test";
import { createOrganization } from "./api";
import { axeInBothThemes } from "./axe";
import { VERSION_KEY, expect, seed, signedIn, test } from "./fixtures";

const { hashbrown, version_zero } = seed.projects;

const billingPath = (organization: string) =>
	`/next/console/organizations/${organization}/billing`;
const classicBilling = (organization: string) =>
	`/console/organizations/${organization}/billing`;
const heading = (page: Page) =>
	page.getByRole("heading", { level: 1, name: "Billing" });
const keyBox = (page: Page) => page.locator('[aria-labelledby="sec-key"]');

test.use({ storageState: signedIn(seed.member) });

test.describe("Billing", () => {
	// Kills a licensed install drawn without its license, or with the key
	// before the organization's UUID.
	test("a licensed organization shows its license, then its UUID and key", async ({
		page,
	}) => {
		await page.goto(billingPath(seed.organization.slug));

		await expect(
			page.getByRole("region", { name: "Enterprise License" }),
		).toContainText("Metrics this term");
		await expect(keyBox(page).locator(".formrow").first()).toContainText(
			seed.organization.uuid,
		);
		await expect(
			page.getByRole("textbox", { name: "License key" }),
		).not.toHaveValue("");
		await expect(
			page.getByRole("button", { name: "Replace key" }),
		).toBeDisabled();
	});

	// Kills an unlicensed install drawn without this month's usage or Contact
	// us, and a key the API refuses without saying why.
	test("an unlicensed organization offers a key, and a refused one says why", async ({
		page,
		request,
	}) => {
		const organization = await createOrganization(request);
		await page.goto(billingPath(organization.slug));

		await expect(
			page.getByRole("region", { name: "This month" }),
		).toContainText("Metrics");
		await expect(
			page
				.getByRole("region", { name: "License", exact: true })
				.getByRole("link", { name: "Contact us" }),
		).toBeVisible();
		await page
			.getByRole("textbox", { name: "License key" })
			.fill("not.a.license");
		await page.getByRole("button", { name: "Save key" }).click();

		await expect(keyBox(page).getByRole("alert")).toContainText(
			"Bencher did not save the license key",
		);
	});
});

test.describe("from General", () => {
	// Kills a rail whose Billing leaves the app for a full page load, or for
	// the classic console.
	test("the rail opens Billing in the app", async ({ page }) => {
		await page.goto(
			`/next/console/organizations/${seed.organization.slug}/settings`,
		);
		await expect(page.getByRole("textbox", { name: "Name" })).toHaveValue(
			seed.organization.name,
		);
		await page.evaluate(() => {
			(window as unknown as { stayed: boolean }).stayed = true;
		});
		await page
			.getByRole("navigation", { name: "Organization settings", exact: true })
			.getByRole("link", { name: "Billing" })
			.click();

		await expect(page).toHaveURL(billingPath(seed.organization.slug));
		await expect(heading(page)).toBeVisible();
		expect(
			await page.evaluate(
				() => (window as unknown as { stayed?: boolean }).stayed,
			),
		).toBe(true);
	});
});

const remembered = (versions: Record<string, number>) =>
	signedIn(seed.member, { [VERSION_KEY]: JSON.stringify(versions) });

test.describe("the classic Billing, for a reader with a version 1 project", () => {
	test.use({ storageState: remembered({ [hashbrown.slug]: 1 }) });

	// Kills a classic Billing that drops the pricing page's plan on the way.
	test("keeps the query on the way", async ({ page }) => {
		await page.goto(`${classicBilling(seed.organization.slug)}?plan=pro`);

		await expect(page).toHaveURL(
			`${billingPath(seed.organization.slug)}?plan=pro`,
		);
	});

	// Kills a classic Billing that keeps a reader the new one serves.
	test("sends the reader to Billing under /next/", async ({ page }) => {
		await page.goto(classicBilling(seed.organization.slug));

		await expect(page).toHaveURL(billingPath(seed.organization.slug));
		await expect(heading(page)).toBeVisible();
	});
});

test.describe("the classic Billing, for a reader of version 0 projects only", () => {
	test.use({ storageState: remembered({ [version_zero.slug]: 0 }) });

	// Kills a classic Billing that sends away a reader the new one does not serve.
	test("keeps the reader", async ({ page }) => {
		await page.goto(classicBilling(seed.organization.slug));
		await page.waitForLoadState("networkidle");

		await expect(page).toHaveURL(classicBilling(seed.organization.slug));
	});
});

test.describe("accessibility", () => {
	// Kills contrast, names, and roles that fail axe in either theme.
	test("a licensed organization's Billing passes axe in both themes", async ({
		page,
	}) => {
		await axeInBothThemes(page, billingPath(seed.organization.slug), () =>
			expect(
				page.getByRole("textbox", { name: "License key" }),
			).not.toHaveValue(""),
		);
	});

	test("an unlicensed organization's Billing passes axe in both themes", async ({
		page,
		request,
	}) => {
		const organization = await createOrganization(request);
		await axeInBothThemes(page, billingPath(organization.slug), () =>
			expect(page.getByRole("button", { name: "Save key" })).toBeVisible(),
		);
	});
});
