import type { Page } from "@playwright/test";
import type { JsonMember } from "../src/types/bencher";
import { createMember, createOrganization, unique } from "./api";
import { axeInBothThemes } from "./axe";
import { VERSION_KEY, expect, seed, signedIn, test } from "./fixtures";

const { hashbrown, version_zero } = seed.projects;

const membersPath = (organization: string) =>
	`/next/console/organizations/${organization}/members`;
const classicMembers = (organization: string) =>
	`/console/organizations/${organization}/members`;
const heading = (page: Page) =>
	page.getByRole("heading", { level: 1, name: "Members" });
const table = (page: Page) => page.getByRole("table", { name: "Members" });

test.use({ storageState: signedIn(seed.member) });

test.describe("Members", () => {
	// Kills an invite that never reaches the API, goes without the Leader role,
	// says nothing once sent, or lists someone who has not yet accepted.
	test("an invite reaches the API and says it was sent, adding no row", async ({
		page,
		request,
	}) => {
		const organization = await createOrganization(request);
		await page.goto(membersPath(organization.slug));
		await expect(table(page).getByRole("row")).toHaveCount(2);
		const email = `${unique("courage").replace(" ", "-")}@nowhere.example`;

		await page.getByRole("textbox", { name: "Email" }).fill(email);
		const [sent] = await Promise.all([
			page.waitForResponse(
				(response) =>
					response.request().method() === "POST" &&
					response.url() ===
						`${seed.api_url}/v0/organizations/${organization.slug}/members`,
			),
			page.getByRole("button", { name: "Send invite" }).click(),
		]);

		expect(sent.status()).toBe(202);
		expect(sent.request().postDataJSON()).toEqual({ email, role: "leader" });
		await expect(page.getByRole("main").getByRole("status")).toHaveText(
			`Invitation sent to ${email}`,
		);
		// The header row and the reader's own.
		await expect(table(page).getByRole("row")).toHaveCount(2);
	});

	// Kills a remove that skips its confirmation, never reaches the API, or
	// leaves the member listed.
	test("removing a member confirms, then the API no longer lists them", async ({
		page,
		request,
	}) => {
		const organization = await createOrganization(request);
		const member = await createMember(request, organization);
		const name = member.user.name;
		await page.goto(membersPath(organization.slug));

		await page.getByRole("button", { name: `Remove ${name}` }).click();
		await page
			.getByRole("alertdialog", { name: `Remove ${name}?` })
			.getByRole("button", { name: "Remove member" })
			.click();

		await expect(
			page.getByRole("button", { name: `Remove ${name}` }),
		).toHaveCount(0);
		const listed = (await (
			await request.get(
				`${seed.api_url}/v0/organizations/${organization.slug}/members`,
				{ headers: { Authorization: `Bearer ${seed.member.token}` } },
			)
		).json()) as JsonMember[];
		expect(listed.map(({ uuid }) => uuid)).toEqual([seed.member.user.uuid]);
	});
});

test.describe("from General", () => {
	// Kills a rail that leaves the console's app for Members, reloading the
	// page, or a title left on General.
	test("the rail opens Members without a reload", async ({ page }) => {
		await page.goto(
			`/next/console/organizations/${seed.organization.slug}/settings`,
		);
		await expect(page.getByRole("textbox", { name: "Name" })).toHaveValue(
			seed.organization.name,
		);
		await page.evaluate(() => {
			(window as Window & { stayed?: boolean }).stayed = true;
		});

		await page
			.getByRole("navigation", { name: "Organization settings", exact: true })
			.getByRole("link", { name: "Members" })
			.click();

		await expect(page).toHaveURL(membersPath(seed.organization.slug));
		await expect(
			table(page).getByRole("row", { name: seed.member.user.name }),
		).toBeVisible();
		await expect(page).toHaveTitle(
			`Members | ${seed.organization.name} | Bencher`,
		);
		expect(
			await page.evaluate(
				() => (window as Window & { stayed?: boolean }).stayed,
			),
		).toBe(true);
	});
});

const remembered = (versions: Record<string, number>) =>
	signedIn(seed.member, { [VERSION_KEY]: JSON.stringify(versions) });

test.describe("the classic members, for a reader with a version 1 project", () => {
	test.use({ storageState: remembered({ [hashbrown.slug]: 1 }) });

	// Kills a classic members page that keeps a reader the new one serves.
	test("sends the reader to Members under /next/", async ({ page }) => {
		await page.goto(classicMembers(seed.organization.slug));

		await expect(page).toHaveURL(membersPath(seed.organization.slug));
		await expect(heading(page)).toBeVisible();
	});
});

test.describe("the classic members, for a reader of version 0 projects only", () => {
	test.use({ storageState: remembered({ [version_zero.slug]: 0 }) });

	// Kills a classic members page that sends away a reader the new one does not serve.
	test("keeps the reader", async ({ page }) => {
		await page.goto(classicMembers(seed.organization.slug));
		await page.waitForLoadState("networkidle");

		expect(new URL(page.url()).pathname).toBe(
			classicMembers(seed.organization.slug),
		);
	});
});

test.describe("accessibility", () => {
	// Kills contrast, names, and roles that fail axe in either theme.
	test("Members passes axe in both themes", async ({ page }) => {
		await axeInBothThemes(page, membersPath(seed.organization.slug), () =>
			expect(
				page.getByRole("region", { name: "Single sign-on" }),
			).toBeVisible(),
		);
	});

	test("the remove confirmation passes axe in both themes", async ({
		page,
		request,
	}) => {
		const organization = await createOrganization(request);
		const member = await createMember(request, organization);
		await axeInBothThemes(
			page,
			membersPath(organization.slug),
			async () => {},
			async () => {
				await page
					.getByRole("button", { name: `Remove ${member.user.name}` })
					.click();
				await expect(page.getByRole("alertdialog")).toBeVisible();
			},
		);
	});
});
