import { afterEach, describe, expect, test } from "vitest";
import { page, userEvent } from "vitest/browser";
import {
	type JsonMember,
	type JsonOrganization,
	OrganizationRole,
} from "../../types/bencher";
import { ApiError } from "../api";
import {
	type MemberPage,
	type Roles,
	membersQuery,
	rolesQuery,
} from "../organization/members";
import { ORGANIZATION, mount } from "../organization/testing";
import { fakeApi } from "../settings/testing";
import OrgMembers from "./OrgMembers";

let dispose: (() => void) | undefined;

afterEach(() => {
	dispose?.();
	dispose = undefined;
});

const member = (uuid: string, name: string, email: string): JsonMember => ({
	uuid,
	name,
	slug: name.toLowerCase().replace(" ", "-"),
	email,
	role: OrganizationRole.Leader,
	created: "2026-09-01T00:00:00Z",
	modified: "2026-09-01T00:00:00Z",
});
// The reader is user "u".
const MURIEL = member("u", "Muriel Bagge", "muriel@nowhere.example");
const EUSTACE = member("u2", "Eustace Bagge", "eustace@nowhere.example");
const LEADER: Roles = { invite: true, remove: true };

/** Members of Pompeii LLC, the API answering each change with `answer`. */
const open = ({
	organization = ORGANIZATION,
	members = { members: [MURIEL, EUSTACE], total: 2 },
	roles = LEADER,
	answer = async () => undefined,
}: {
	organization?: JsonOrganization | null;
	members?: MemberPage;
	roles?: Roles | null;
	answer?: Parameters<typeof fakeApi>[0];
} = {}) => {
	const reads: string[] = [];
	const { api, sent } = fakeApi(answer, (path) => {
		reads.push(path);
		return new Promise<never>(() => {});
	});
	const key = "pompeii-llc";
	const cached: [unknown[], unknown][] = [
		[membersQuery(api, key).queryKey, members],
	];
	if (roles) {
		cached.push([rolesQuery(api, key).queryKey, roles]);
	}
	const mounted = mount(() => <OrgMembers />, {
		api,
		path: `/organizations/${key}/members`,
		organization,
		cached,
	});
	dispose = mounted.dispose;
	return { ...mounted, sent, reads };
};

const table = () => page.getByRole("table", { name: "Members" });
const rows = () =>
	[...document.querySelectorAll("tbody tr")].map((row) =>
		[...row.querySelectorAll("td")].map((cell) => cell.textContent?.trim()),
	);
const email = () => page.getByRole("textbox", { name: "Email" });
const send = () => page.getByRole("button", { name: "Send invite" });

// Kills a Remove on the reader's own row or missing from another's, the
// reader's row unmarked, and a count of the rows drawn rather than the API's.
test("lists each member with Remove, and the reader's own row without it", async () => {
	open({ members: { members: [MURIEL, EUSTACE], total: 260 } });
	await expect.element(table()).toBeVisible();
	expect(rows()).toEqual([
		["Muriel Bagge you", "muriel@nowhere.example", ""],
		["Eustace Bagge", "eustace@nowhere.example", "Remove"],
	]);
	await expect
		.element(page.getByRole("button", { name: "Remove Eustace Bagge" }))
		.toBeVisible();
	expect(
		page.getByRole("button", { name: "Remove Muriel Bagge" }).query(),
	).toBeNull();
	await expect.element(page.getByText("260 members")).toBeVisible();
});

describe("invite", () => {
	// Kills an invite sent elsewhere, without the Leader role, or before the
	// address has a name before its @; a row added for someone who has not yet
	// accepted; and a field that keeps the address once it is sent.
	test("sends the address as a Leader and says so, adding no row", async () => {
		const { sent } = open({
			answer: async () => ({ email: "courage@nowhere.example" }),
		});
		await email().fill("courage");
		await expect.element(send()).toBeDisabled();
		await email().fill("courage@nowhere.example");
		await send().click();

		await expect
			.element(page.getByRole("status"))
			.toHaveTextContent("Invitation sent to courage@nowhere.example");
		expect(sent).toEqual([
			{
				method: "POST",
				path: "/v0/organizations/pompeii-llc/members",
				body: { email: "courage@nowhere.example", role: "leader" },
			},
		]);
		await expect.element(email()).toHaveValue("");
		expect(rows()).toHaveLength(2);
	});

	// Kills a second invite sent while the first is still on its way.
	test("Enter again while an invite is on its way sends nothing more", async () => {
		const { sent } = open({ answer: () => new Promise(() => {}) });
		await email().fill("courage@nowhere.example");
		await userEvent.keyboard("{Enter}");
		await expect.element(send()).toBeDisabled();
		await userEvent.keyboard("{Enter}");

		expect(sent).toHaveLength(1);
	});

	// Kills a refused invite that throws the draft away, says nothing, or
	// hides what the API said.
	test("a refused invite says why and keeps the address", async () => {
		open({
			answer: async () => {
				throw new ApiError(400, "client", '{"message":"Invalid email"}');
			},
		});
		await email().fill("courage@nowhere.example");
		await send().click();

		await expect
			.element(page.getByRole("alert"))
			.toHaveTextContent("Bencher did not send the invitation: Invalid email");
		await expect.element(email()).toHaveValue("courage@nowhere.example");
	});
});

describe("remove", () => {
	// Kills a remove sent without confirming, a Cancel that sends it, a remove
	// for another member, a row or count left after it, focus lost with the
	// row, and a list read again rather than kept.
	test("confirms, sends, then drops the row", async () => {
		const { sent, reads } = open();
		const remove = page.getByRole("button", { name: "Remove Eustace Bagge" });
		await remove.click();
		const dialog = page.getByRole("alertdialog", {
			name: "Remove Eustace Bagge?",
		});
		await expect
			.element(dialog)
			.toHaveAccessibleDescription(
				"Eustace Bagge loses access to every project in Pompeii LLC.",
			);
		await dialog.getByRole("button", { name: "Cancel" }).click();
		await expect.element(dialog).not.toBeInTheDocument();
		expect(sent).toEqual([]);

		await remove.click();
		await dialog.getByRole("button", { name: "Remove member" }).click();

		await expect.element(remove).not.toBeInTheDocument();
		expect(sent).toEqual([
			{
				method: "DELETE",
				path: "/v0/organizations/pompeii-llc/members/u2",
				body: undefined,
			},
		]);
		expect(rows()).toEqual([
			["Muriel Bagge you", "muriel@nowhere.example", ""],
		]);
		await expect.element(page.getByText("1 member")).toBeVisible();
		await expect
			.element(page.getByRole("status"))
			.toHaveTextContent("Removed Eustace Bagge");
		expect(reads).toEqual([]);
		await expect
			.element(page.getByRole("heading", { level: 1, name: "Members" }))
			.toHaveFocus();
	});

	// Kills a row dropped though the API refused, and a refusal that says nothing.
	test("a refused remove keeps the row and says why", async () => {
		open({
			answer: async () => {
				throw new ApiError(403, "forbidden", '{"message":"Forbidden"}');
			},
		});
		await page.getByRole("button", { name: "Remove Eustace Bagge" }).click();
		await page
			.getByRole("alertdialog")
			.getByRole("button", { name: "Remove member" })
			.click();

		await expect
			.element(page.getByRole("alert"))
			.toHaveTextContent("Bencher did not remove Eustace Bagge: Forbidden");
		expect(rows()).toHaveLength(2);
	});
});

describe("single sign-on", () => {
	const box = () => page.getByRole("region", { name: "Single sign-on" });

	// Kills an organization's domains left out, or offered Enterprise anyway.
	test("an organization with single sign-on lists its domains", async () => {
		open({
			organization: {
				...ORGANIZATION,
				sso: [
					{
						uuid: "s1",
						domain: "tumbleweed.example",
						created: "2026-03-02T12:00:00Z",
					},
				],
			},
		});
		await expect.element(box()).toHaveTextContent("1 domain");
		await expect.element(box()).toHaveTextContent("tumbleweed.example");
		await expect.element(box()).toHaveTextContent("Added Mar 2, 2026");
		expect(box().getByRole("link", { name: "Contact us" }).query()).toBeNull();
	});

	// Kills the box hidden from an organization without single sign-on, or
	// drawn without its way to ask for it.
	test("an organization without it is offered Enterprise", async () => {
		open();
		await expect.element(box()).toHaveTextContent("Available on Enterprise");
		await expect
			.element(box().getByRole("link", { name: "Contact us" }))
			.toHaveAttribute("href", "mailto:enterprise@bencher.dev");
	});

	// Kills a page that draws from the reader's bootstrap, which carries no
	// single sign-on, before the organization itself arrives.
	test("nothing is drawn before the organization arrives", async () => {
		open({ organization: null });
		await expect
			.element(page.getByRole("heading", { level: 1, name: "Members" }))
			.toBeVisible();
		expect(box().query()).toBeNull();
		expect(table().query()).toBeNull();
	});
});

describe("only what the reader can do", () => {
	const shown = () => ({
		invite: document.querySelectorAll("#inv-email").length,
		remove: document.querySelectorAll("td.act button").length,
	});

	// Kills invite and remove read as one permission, or drawn for a reader
	// the API would refuse.
	test("Invite and Remove each follow their own permission", async () => {
		open({ roles: { invite: false, remove: false } });
		await expect.element(table()).toBeVisible();
		expect(shown()).toEqual({ invite: 0, remove: 0 });
		dispose?.();

		open({ roles: { invite: true, remove: false } });
		await expect.element(table()).toBeVisible();
		expect(shown()).toEqual({ invite: 1, remove: 0 });
		dispose?.();

		open({ roles: { invite: false, remove: true } });
		await expect.element(table()).toBeVisible();
		expect(shown()).toEqual({ invite: 0, remove: 1 });
	});

	// Kills controls drawn before the API has said what the reader may do.
	test("nothing is drawn until the permissions are known", async () => {
		open({ roles: null });
		await expect
			.element(page.getByRole("heading", { level: 1, name: "Members" }))
			.toBeVisible();
		expect(shown()).toEqual({ invite: 0, remove: 0 });
		expect(table().query()).toBeNull();
	});
});

// Kills a rail that leaves General current, or Members leading anywhere but here.
test("the rail marks Members current", async () => {
	await page.viewport(1280, 800);
	open();
	const link = page
		.getByRole("navigation", { name: "Organization settings", exact: true })
		.getByRole("link", { name: "Members" });
	await expect.element(link).toHaveAttribute("aria-current", "page");
	await expect
		.element(link)
		.toHaveAttribute("href", "/console/organizations/pompeii-llc/members");
});
