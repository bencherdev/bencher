import { randomUUID } from "node:crypto";
import type { Page } from "@playwright/test";
import type { JsonOrganization } from "../src/types/bencher";
import { createOrganization, createReader, invite } from "./api";
import { axeInBothThemes } from "./axe";
import { continueWithEmail, pasteCode } from "./door";
import {
	SESSION_COOKIE,
	expect,
	seed,
	signedIn,
	signedOut,
	test,
} from "./fixtures";
import { mailedLink } from "./mail";
import { inSession } from "./session";

const newEmail = () => `invited-${randomUUID().slice(0, 8)}@nowhere.example`;
const heading = (page: Page, name: string | RegExp) =>
	page.getByRole("heading", { level: 1, name });
const membersPath = (organization: string) =>
	`/console/organizations/${organization}/members`;

/**
 * Open `href` as a page of its own, as a link from an email does: a tab
 * already at its path would hear only a change of fragment.
 */
const opened = (page: Page, href: string) => async () => {
	await page.goto("about:blank");
	await page.goto(href);
};

/** Every request URL the page sends from now on. */
const sentUrls = (page: Page) => {
	const urls: string[] = [];
	page.on("request", (sent) => urls.push(sent.url()));
	return urls;
};

/** The organizations the session's reader belongs to. */
const organizationsOf = async (session: string) =>
	(
		await inSession<JsonOrganization[]>({ session }, "GET", "/v0/organizations")
	).data.map(({ slug }) => slug);

test.describe("signed out", () => {
	test.use({ storageState: signedOut });

	// Kills an invitation whose address the reader must type again, a secret
	// left in the address bar or sent in any request's URL, a door that reads
	// the invitation from anything but the API, and a sign up that does not
	// join the organization.
	test("the link fills in the address, and a sign up by code joins", async ({
		page,
		context,
		request,
	}) => {
		const organization = await createOrganization(request);
		const email = newEmail();
		const { link, secret } = await invite(seed.member, organization, email);
		expect(link.pathname).toBe("/auth/login");
		expect(secret).not.toBe("");
		const sent = sentUrls(page);

		await page.goto(link.href);
		await expect(page.getByRole("textbox", { name: "Email" })).toHaveValue(
			email,
		);
		await expect(page).toHaveURL("/auth/login");
		const { code } = await continueWithEmail(page, email);
		await pasteCode(page, code);
		await expect(page).toHaveURL(/\/console\//);

		expect(sent.filter((url) => url.includes(secret))).toEqual([]);
		const session =
			(await context.cookies()).find(({ name }) => name === SESSION_COOKIE)
				?.value ?? "";
		expect(await organizationsOf(session)).toContain(organization.slug);
	});

	// Kills an old link that still works once the invitation was sent again, a
	// revoked invitation that still works, and a refusal that blocks signing in.
	test("a resent invitation's old link and a revoked one read as not valid", async ({
		page,
		request,
	}) => {
		const organization = await createOrganization(request);
		const email = newEmail();
		const first = await invite(seed.member, organization, email);
		const [pending] = (
			await inSession<{ uuid: string }[]>(
				seed.member,
				"GET",
				`/v0/organizations/${organization.slug}/invites`,
			)
		).data;
		const resent = await inSession(
			seed.member,
			"POST",
			`/v0/organizations/${organization.slug}/invites/${pending?.uuid}/resend`,
		);
		expect(resent.status).toBe(202);
		const second = await mailedLink(
			email,
			`Invitation to join ${organization.name}`,
		);
		expect(second.hash).not.toBe(first.link.hash);

		await page.goto(first.link.href);
		await expect(page.getByRole("alert")).toHaveText(
			"This invite is not valid",
		);
		await expect(page.getByRole("textbox", { name: "Email" })).toHaveValue("");

		const revoked = await inSession(
			seed.member,
			"DELETE",
			`/v0/organizations/${organization.slug}/invites/${pending?.uuid}`,
		);
		expect(revoked.status).toBe(204);
		await page.goto(second.href);
		await expect(page.getByRole("alert")).toHaveText(
			"This invite is not valid",
		);
		// The refusal leaves the door, which signs the reader in all the same.
		await continueWithEmail(page, email);
	});

	test("the door with an invitation's address passes axe in both themes", async ({
		page,
		request,
	}) => {
		const organization = await createOrganization(request);
		const email = newEmail();
		const { link } = await invite(seed.member, organization, email);
		await axeInBothThemes(
			page,
			link.href,
			() =>
				expect(page.getByRole("textbox", { name: "Email" })).toHaveValue(email),
			opened(page, link.href),
		);
	});
});

test.describe("signed in", () => {
	// Kills an invitation accepted as the page loads, with no press, a Join
	// that does not join or lands anywhere but the organization, and a press
	// that names the wrong organization or role.
	test("an invitation for the reader asks first, and Join joins", async ({
		newContext,
		request,
	}) => {
		const organization = await createOrganization(request);
		const reader = await createReader();
		const { link } = await invite(seed.member, organization, reader.user.email);
		const page = await (
			await newContext({ storageState: signedIn(reader) })
		).newPage();

		await page.goto(link.href);
		await expect(
			heading(page, `Join ${organization.name} as Leader`),
		).toBeVisible();
		await expect(page).toHaveURL("/auth/login");
		expect(await organizationsOf(reader.session)).not.toContain(
			organization.slug,
		);
		await expect(page.getByRole("link", { name: "Cancel" })).toHaveAttribute(
			"href",
			"/console",
		);

		await page.getByRole("button", { name: "Join", exact: true }).click();
		await expect(page).toHaveURL(
			`/console/organizations/${organization.slug}/settings`,
		);
		expect(await organizationsOf(reader.session)).toContain(organization.slug);
	});

	// Kills an invitation for another address that joins the reader, signs
	// them out unasked, or leaves the door without the invited address.
	test("an invitation for another address offers Sign out, then the door", async ({
		newContext,
		request,
	}) => {
		const organization = await createOrganization(request);
		const reader = await createReader();
		const email = newEmail();
		const { link } = await invite(seed.member, organization, email);
		const page = await (
			await newContext({ storageState: signedIn(reader) })
		).newPage();

		await page.goto(link.href);
		await expect(heading(page, `This invite is for ${email}`)).toBeVisible();
		expect((await inSession(reader, "GET", "/v0/auth/session")).status).toBe(
			200,
		);

		await page.getByRole("button", { name: "Sign out" }).click();
		await expect(page.getByRole("textbox", { name: "Email" })).toHaveValue(
			email,
		);
		await expect(heading(page, "Sign in to Bencher")).toBeVisible();
		expect((await inSession(reader, "GET", "/v0/auth/session")).status).toBe(
			401,
		);
		expect(await organizationsOf(seed.member.session)).toContain(
			organization.slug,
		);
	});

	// Kills a hint of a reader whose session has ended that leaves the door
	// without the invited address or reads the invitation twice.
	test("an ended session's reader gets the door with the address, read once", async ({
		newContext,
		request,
	}) => {
		const organization = await createOrganization(request);
		const reader = await createReader();
		const { link } = await invite(seed.member, organization, reader.user.email);
		const page = await (
			await newContext({ storageState: signedIn(reader) })
		).newPage();
		expect(
			(await inSession(reader, "DELETE", "/v0/auth/session")).status,
		).toBeLessThan(300);
		const sent = sentUrls(page);

		await page.goto(link.href);
		await expect(page.getByRole("textbox", { name: "Email" })).toHaveValue(
			reader.user.email,
		);
		await expect(heading(page, "Sign in to Bencher")).toBeVisible();
		expect(sent.filter((url) => url.endsWith("/v0/auth/invite"))).toHaveLength(
			1,
		);
	});

	test("each press passes axe in both themes", async ({
		newContext,
		request,
	}) => {
		const organization = await createOrganization(request);
		const reader = await createReader();
		const page = await (
			await newContext({ storageState: signedIn(reader) })
		).newPage();
		const own = await invite(seed.member, organization, reader.user.email);
		await axeInBothThemes(
			page,
			own.link.href,
			() =>
				expect(
					heading(page, `Join ${organization.name} as Leader`),
				).toBeVisible(),
			opened(page, own.link.href),
		);
		const email = newEmail();
		const other = await invite(seed.member, organization, email);
		await page.goto("about:blank");
		await axeInBothThemes(
			page,
			other.link.href,
			() => expect(heading(page, `This invite is for ${email}`)).toBeVisible(),
			opened(page, other.link.href),
		);
		await page.goto("about:blank");
		await axeInBothThemes(
			page,
			`/auth/login#invite=${"A".repeat(43)}`,
			() => expect(heading(page, "This invite is not valid")).toBeVisible(),
			opened(page, `/auth/login#invite=${"A".repeat(43)}`),
		);
	});
});

test.describe("Members", () => {
	test.use({ storageState: signedIn(seed.member) });

	// Kills an invitation the card does not list once sent, a Resend that sends
	// no new email or says nothing, and a Revoke that leaves the row or its link.
	test("the card lists an invitation once sent, resends it, and revokes it", async ({
		page,
		request,
	}) => {
		const organization = await createOrganization(request);
		const email = newEmail();
		await page.goto(membersPath(organization.slug));
		await page.getByRole("textbox", { name: "Email" }).fill(email);
		await page.getByRole("button", { name: "Send invite" }).click();
		const status = page.getByRole("main").getByRole("status");
		await expect(status).toHaveText(`Invitation sent to ${email}`);
		const card = page.getByRole("region", { name: "Pending invites" });
		const row = card.getByRole("row", { name: new RegExp(email) });
		await expect(row).toContainText(seed.member.user.name);
		await expect(row).toContainText(/Expires/);
		const sent = await invite(seed.member, organization, email);

		await row
			.getByRole("button", { name: `Resend the invitation to ${email}` })
			.click();
		await expect(status).toHaveText(`Invitation sent to ${email}`);
		await expect
			.poll(
				async () =>
					(
						await inSession(seed.member, "POST", "/v0/auth/invite", {
							secret: sent.secret,
						})
					).status,
			)
			.toBe(400);

		await row
			.getByRole("button", { name: `Revoke the invitation to ${email}` })
			.click();
		await expect(status).toHaveText(`Invitation to ${email} revoked`);
		await expect(card).toHaveCount(0);
	});

	test("the card passes axe in both themes", async ({ page, request }) => {
		const organization = await createOrganization(request);
		await invite(seed.member, organization, newEmail());
		await axeInBothThemes(page, membersPath(organization.slug), () =>
			expect(
				page.getByRole("table", { name: "Pending invites" }),
			).toBeVisible(),
		);
	});
});
