import { render } from "solid-js/web";
import { afterEach, describe, expect, test } from "vitest";
import { page, userEvent } from "vitest/browser";
import {
	type JsonInvite,
	type JsonMember,
	type JsonOrganization,
	OrganizationRole,
} from "../../types/bencher";
import { ApiError } from "../api";
import { axeInBothThemes } from "../axe/testing";
import { screenBatch } from "../list/rows";
import {
	type Roles,
	invitesQuery,
	membersKey,
	rolesQuery,
} from "../organization/members";
import { ORGANIZATION, mount } from "../organization/testing";
import { KeyTable } from "../settings/KeyList";
import { fakeApi } from "../settings/testing";
import {
	FLOOR_WIDTHS,
	STRESS,
	expectFoldAt,
	expectLinesAt,
	nameFloor,
	sizeTo,
} from "../ui/testing";
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

const pending = (
	uuid: string,
	email: string,
	inviter: string,
	expiration: string,
): JsonInvite => ({
	uuid,
	email,
	role: OrganizationRole.Leader,
	inviter: { uuid: `u-${inviter}`, name: inviter, slug: inviter.toLowerCase() },
	created: "2026-10-20T00:00:00Z",
	expiration,
});
const GRACE = pending(
	"i1",
	"grace@nowhere.example",
	"Muriel Bagge",
	"2999-11-05T12:00:00Z",
);
const KATHERINE = pending(
	"i2",
	"katherine@nowhere.example",
	"Eustace Bagge",
	"2026-10-27T12:00:00Z",
);

interface MemberPage {
	members: JsonMember[];
	total: number;
}

/** Members of Pompeii LLC, the API answering each change with `answer`. */
const open = ({
	organization = ORGANIZATION,
	members = { members: [MURIEL, EUSTACE], total: 2 },
	roles = LEADER,
	invites = { invites: [], total: 0 },
	answer = async () => undefined,
	read = () => new Promise<never>(() => {}),
}: {
	organization?: JsonOrganization | null;
	/** The first page the cache holds, or none. */
	members?: MemberPage | null;
	roles?: Roles | null;
	/** The pending invitations the cache holds, or none. */
	invites?: { invites: JsonInvite[]; total: number } | null;
	answer?: Parameters<typeof fakeApi>[0];
	read?: (path: string) => Promise<never>;
} = {}) => {
	const reads: string[] = [];
	const { api, sent } = fakeApi(answer, (path) => {
		reads.push(path);
		return read(path);
	});
	const key = "pompeii-llc";
	const cached: [unknown[], unknown][] = members
		? [
				[
					[...membersKey(key), screenBatch(), 1],
					{ rows: members.members, total: members.total, page: 1 },
				],
			]
		: [];
	if (roles) {
		cached.push([rolesQuery(api, key).queryKey, roles]);
	}
	if (invites) {
		cached.push([invitesQuery(api, key).queryKey, invites]);
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

// Kills a list that stops at its first page, one that draws every member it
// has read, and a count of the rows read rather than the API's.
test("a long list reads on as the reader nears its end, drawing only the rows on screen", async () => {
	await page.viewport(1280, 800);
	const all = Array.from({ length: 300 }, (_, n) =>
		member(`m${n}`, `Member ${n}`, `m${n}@nowhere.example`),
	);
	const { reads } = open({
		members: null,
		read: async (path) => {
			const url = new URL(path, "http://api.test");
			if (!url.pathname.endsWith("/members")) {
				return new Promise<never>(() => {});
			}
			const size = Number(url.searchParams.get("per_page"));
			const at = Number(url.searchParams.get("page"));
			return {
				data: all.slice((at - 1) * size, at * size),
				headers: new Headers({ "X-Total-Count": "300" }),
			} as never;
		},
	});
	await expect.element(table()).toBeVisible();
	await expect.element(page.getByText("300 members")).toBeVisible();
	const pages = () => reads.filter((path) => path.includes("/members?")).length;
	expect(pages()).toBe(1);
	for (
		let step = 0;
		step < 40 && !page.getByText("Member 299").query();
		step++
	) {
		window.scrollTo({
			top: document.documentElement.scrollHeight,
			behavior: "instant",
		});
		await new Promise((resolve) => setTimeout(resolve, 50));
		expect(
			document.querySelectorAll("tbody tr[aria-rowindex]").length,
		).toBeLessThan(80);
	}
	await expect.element(page.getByText("Member 299")).toBeVisible();
	expect(pages()).toBe(Math.ceil(300 / screenBatch()));
	expect(table().element().getAttribute("aria-rowcount")).toBe("301");
});

// Kills a member's name opened in place that closes, and moves the rows on
// screen, once the reader scrolls far down the list.
test("a member's name opened in place stays open as the list scrolls away", async () => {
	await page.viewport(390, 844);
	const all = Array.from({ length: 300 }, (_, n) =>
		member(
			`m${n}`,
			n === 0 ? "M".repeat(64) : `Member ${n}`,
			`m${n}@nowhere.example`,
		),
	);
	open({
		members: null,
		read: async (path) => {
			const url = new URL(path, "http://api.test");
			if (!url.pathname.endsWith("/members")) {
				return new Promise<never>(() => {});
			}
			const size = Number(url.searchParams.get("per_page"));
			const at = Number(url.searchParams.get("page"));
			return {
				data: all.slice((at - 1) * size, at * size),
				headers: new Headers({ "X-Total-Count": "300" }),
			} as never;
		},
	});
	await expect.element(table()).toBeVisible();
	await expect
		.poll(() => document.querySelector("tbody button.ui-trunc"))
		.not.toBeNull();
	document.querySelector<HTMLElement>("tbody button.ui-trunc")?.click();
	await new Promise((resolve) =>
		requestAnimationFrame(() => setTimeout(resolve, 0)),
	);
	for (let step = 0; step < 60; step++) {
		window.scrollBy({ top: 300, behavior: "instant" });
		await new Promise((resolve) =>
			requestAnimationFrame(() => setTimeout(resolve, 10)),
		);
	}
	expect(
		document.querySelector("tbody .ui-trunc[data-open]")?.textContent,
	).toBe("M".repeat(64));
	window.scrollTo({ top: 0, behavior: "instant" });
});

// Kills a Remove on the reader's own row or missing from another's, a Remove
// named by a name two members can share, the reader's row unmarked, and a
// count of the rows drawn rather than the API's.
test("lists each member with Remove, and the reader's own row without it", async () => {
	open({ members: { members: [MURIEL, EUSTACE], total: 260 } });
	await expect.element(table()).toBeVisible();
	expect(rows()).toEqual([
		["Muriel Bagge you", "muriel@nowhere.example", ""],
		["Eustace Bagge", "eustace@nowhere.example", "Remove"],
	]);
	await expect
		.element(
			page.getByRole("button", {
				name: "Remove Eustace Bagge (eustace@nowhere.example)",
				exact: true,
			}),
		)
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

describe("pending invites", () => {
	const card = () => page.getByRole("region", { name: "Pending invites" });
	const invites = () => page.getByRole("table", { name: "Pending invites" });
	const inviteRows = () =>
		[...(invites().element().querySelectorAll("tbody tr") ?? [])].map((row) =>
			[...row.querySelectorAll("td")].map((cell) => cell.textContent?.trim()),
		);

	// Kills an invitation left off the card, its inviter or its expiry left
	// out, an expired one dropped or read as live, Resend or Revoke missing
	// from any row, and a count of the rows drawn rather than the API's.
	test("lists each invitation with who sent it, when it stops working, Resend, and Revoke", async () => {
		open({ invites: { invites: [GRACE, KATHERINE], total: 3 } });
		await expect.element(invites()).toBeVisible();
		await expect.element(card()).toHaveTextContent("Pending invites3");
		expect(inviteRows()).toEqual([
			[
				"grace@nowhere.example",
				"Invited by Muriel Bagge",
				"Expires Nov 5, 2999",
				"ResendRevoke",
			],
			[
				"katherine@nowhere.example",
				"Invited by Eustace Bagge",
				"Expired Oct 27, 2026",
				"ResendRevoke",
			],
		]);
		expect(
			invites().element().querySelector("tbody tr:last-child .faint")
				?.textContent,
		).toBe("Expired Oct 27, 2026");
		for (const email of [
			"grace@nowhere.example",
			"katherine@nowhere.example",
		]) {
			await expect
				.element(
					page.getByRole("button", {
						name: `Resend the invitation to ${email}`,
					}),
				)
				.toBeVisible();
			await expect
				.element(
					page.getByRole("button", {
						name: `Revoke the invitation to ${email}`,
					}),
				)
				.toBeVisible();
		}
	});

	// Kills a card drawn for a reader who may not invite, or with nothing on it.
	test("only a reader who may invite sees it, and only when one waits", async () => {
		open({
			roles: { invite: false, remove: true },
			invites: { invites: [GRACE], total: 1 },
		});
		await expect.element(table()).toBeVisible();
		expect(card().query()).toBeNull();
		dispose?.();

		open();
		await expect.element(table()).toBeVisible();
		expect(card().query()).toBeNull();
	});

	// The card draws above the members, so members drawn before a late answer
	// would move when it came.
	// Kills a page that draws before its pending invites are in.
	test("nothing draws until the pending invites are in, so their card moves nothing", async () => {
		const late: { answer?: (invites: JsonInvite[]) => void } = {};
		open({
			invites: null,
			read: (path) =>
				new URL(path, "http://api.test").pathname.endsWith("/invites")
					? (new Promise((resolve) => {
							late.answer = (invites) =>
								resolve({
									data: invites,
									headers: new Headers({
										"X-Total-Count": String(invites.length),
									}),
								});
						}) as Promise<never>)
					: new Promise<never>(() => {}),
		});
		await expect.poll(() => late.answer).toBeDefined();
		await new Promise((resolve) => setTimeout(resolve, 100));
		expect(table().query()).toBeNull();
		expect(card().query()).toBeNull();

		late.answer?.([GRACE]);
		await expect.element(card()).toBeVisible();
		await expect.element(table()).toBeVisible();
	});

	// The pending invites are read beside the permissions, so a reader who may
	// not invite is refused them.
	// Kills a refusal that blocks the members or says Members did not load.
	test("a reader refused the pending invites sees the members and nothing else", async () => {
		const { reads } = open({
			roles: { invite: false, remove: false },
			invites: null,
			read: async (path) => {
				if (new URL(path, "http://api.test").pathname.endsWith("/invites")) {
					throw new ApiError(404, "not_found", "{}");
				}
				return new Promise<never>(() => {});
			},
		});
		await expect
			.poll(() => reads.filter((path) => path.includes("/invites")).length)
			.toBe(1);
		await expect.element(table()).toBeVisible();
		expect(card().query()).toBeNull();
		expect(page.getByRole("alert").query()).toBeNull();
	});

	// Kills a Resend or Revoke sent for another invitation, or one that says
	// nothing, and a list left as it was.
	test("Resend and Revoke each send their own change and say what happened", async () => {
		const { sent, reads } = open({
			invites: { invites: [GRACE, KATHERINE], total: 2 },
		});
		await page
			.getByRole("button", {
				name: "Resend the invitation to katherine@nowhere.example",
			})
			.click();
		await expect
			.element(page.getByRole("status"))
			.toHaveTextContent("Invitation sent to katherine@nowhere.example");
		await page
			.getByRole("button", {
				name: "Revoke the invitation to grace@nowhere.example",
			})
			.click();
		await expect
			.element(page.getByRole("status"))
			.toHaveTextContent("Invitation to grace@nowhere.example revoked");
		expect(sent).toEqual([
			{
				method: "POST",
				path: "/v0/organizations/pompeii-llc/invites/i2/resend",
				body: undefined,
			},
			{
				method: "DELETE",
				path: "/v0/organizations/pompeii-llc/invites/i1",
				body: undefined,
			},
		]);
		await expect
			.poll(() => reads.filter((path) => path.includes("/invites?")).length)
			.toBe(2);
	});

	// Kills a refused Resend that says nothing, or says it was sent.
	test("a refused Resend says why", async () => {
		open({
			invites: { invites: [GRACE], total: 1 },
			answer: async () => {
				throw new ApiError(429, "client", '{"message":"Too many invites"}');
			},
		});
		await page
			.getByRole("button", {
				name: "Resend the invitation to grace@nowhere.example",
			})
			.click();
		await expect
			.element(page.getByRole("alert"))
			.toHaveTextContent(
				"Bencher did not send the invitation: Too many invites",
			);
		expect(page.getByText(/^Invitation sent/).query()).toBeNull();
	});

	// Kills contrast, names, and roles that fail axe in either theme, the
	// expired row's faint date among them.
	test("the card passes axe in both themes", async () => {
		await axeInBothThemes(async () => {
			open({ invites: { invites: [GRACE, KATHERINE], total: 2 } });
			await expect.element(invites()).toBeVisible();
			return () => {
				dispose?.();
				dispose = undefined;
			};
		});
	});
});

describe("remove", () => {
	// Kills a remove sent without confirming, a Cancel that sends it, a remove
	// for another member, a confirm that names a member by name alone, a row
	// or count left after it, focus lost with the row, and a list read again
	// rather than kept.
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
				"Eustace Bagge (eustace@nowhere.example) loses access to every project in Pompeii LLC.",
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
		await expect.element(page.getByRole("status")).toHaveTextContent("Removed");
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
			.toHaveTextContent("Not removed: Forbidden");
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

describe("an organization the API will not show the reader", () => {
	// Kills a refusal drawn as a failure to retry, read from the organization
	// or its members.
	test.each([
		[403, "forbidden", "/v0/organizations/pompeii-llc"],
		[404, "not_found", "/v0/organizations/pompeii-llc"],
		[404, "not_found", "/v0/organizations/pompeii-llc/members?"],
	] as const)(
		"a refusal (%i, %s) of %s is not found, with no Retry",
		async (status, kind, refusing) => {
			open({
				organization: null,
				members: null,
				roles: null,
				read: async (path) => {
					if (
						refusing.endsWith("pompeii-llc")
							? path === refusing
							: path.startsWith(refusing)
					) {
						throw new ApiError(status, kind, "{}");
					}
					return new Promise<never>(() => {});
				},
			});
			await expect
				.element(page.getByRole("main").getByText("Organization not found"))
				.toBeVisible();
			expect(page.getByRole("alert").query()).toBeNull();
			expect(page.getByRole("button", { name: "Retry" }).query()).toBeNull();
		},
	);

	// Kills every failure drawn as not found.
	test("a server failure says Members did not load, and retries", async () => {
		const { reads } = open({
			organization: null,
			members: null,
			roles: null,
			read: async () => {
				throw new ApiError(500, "server", "{}");
			},
		});
		await expect
			.element(page.getByRole("alert"))
			.toHaveTextContent("Members did not load");
		expect(page.getByText("Organization not found").query()).toBeNull();
		const before = reads.length;
		await page.getByRole("button", { name: "Retry" }).click();
		await expect.poll(() => reads.length).toBeGreaterThan(before);
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

// Kills a remove confirmation that fails axe in either theme.
test("the remove confirmation passes axe in both themes", async () => {
	await axeInBothThemes(async () => {
		open();
		await page.getByRole("button", { name: "Remove Eustace Bagge" }).click();
		await expect.element(page.getByRole("alertdialog")).toBeVisible();
		return () => {
			dispose?.();
			dispose = undefined;
		};
	});
});

describe("long names", () => {
	const LONG = member(
		"u",
		"a".repeat(64),
		`${"m".repeat(64)}@long-domain-name-for-stress.example`,
	);
	const SPACED = member(
		"u2",
		"Quarterly Platform Performance Regression Tracking Group North X",
		`${"s".repeat(64)}@nowhere.example`,
	);
	/** The name floor, 16ch, in pixels in a table cell. */
	const floor = () => {
		const probe = document.createElement("span");
		probe.style.cssText = "position: absolute; width: var(--size-name-min)";
		document.querySelector("td")?.append(probe);
		const width = probe.getBoundingClientRect().width;
		probe.remove();
		return width;
	};

	// Kills a long name or email that widens the table past its box, a name
	// squeezed under the floor beside a whole email, and "you" pushed out.
	test.each([320, 1024])(
		"at %i px they fit, each name keeps the floor, and you stays whole",
		async (width) => {
			await page.viewport(width, 900);
			open({ members: { members: [LONG, SPACED], total: 2 } });
			await expect.element(table()).toBeVisible();

			const scroll = table().element().parentElement as HTMLElement;
			expect(scroll.scrollWidth).toBeLessThanOrEqual(scroll.clientWidth);
			for (const name of document.querySelectorAll('td[data-fold="l1"] b')) {
				expect(name.getBoundingClientRect().width).toBeGreaterThanOrEqual(
					floor(),
				);
				expect(name.querySelector(".ui-trunc")).not.toBeNull();
				expect(name.scrollWidth).toBeLessThanOrEqual(name.clientWidth);
				expect(name.getBoundingClientRect().height).toBeLessThan(
					Number.parseFloat(getComputedStyle(name).lineHeight) * 1.5,
				);
			}
			const you = document.querySelector<HTMLElement>("td [data-fixed]");
			expect(you?.scrollWidth).toBeLessThanOrEqual(you?.clientWidth ?? 0);
			expect(you?.getBoundingClientRect().right).toBeLessThanOrEqual(
				you?.closest("td")?.getBoundingClientRect().right ?? 0,
			);
		},
	);

	// Kills a long organization name that runs past the rail, or wraps down it.
	test("the rail holds a long organization name on one line", async () => {
		await page.viewport(1440, 900);
		open({ organization: { ...ORGANIZATION, name: "a".repeat(64) } });
		await expect.element(table()).toBeVisible();

		const name = document.querySelector(".rail .eyebrow") as HTMLElement;
		const rail = name.parentElement?.getBoundingClientRect();
		expect(name.getBoundingClientRect().right).toBeLessThanOrEqual(
			rail?.right ?? 0,
		);
		expect(name.getBoundingClientRect().height).toBeLessThan(
			Number.parseFloat(getComputedStyle(name).lineHeight) * 1.5 +
				Number.parseFloat(getComputedStyle(name).paddingTop) * 2,
		);
	});
});

// Kills a members or key list that keeps its columns where its name falls
// under 16 characters, and one that scrolls sideways in its own track,
// pushing Remove or Revoke off its edge.
test("the members and key lists hold 16 characters of name, or fold", async () => {
	const long = member(
		"x",
		STRESS.name,
		`${"m".repeat(64)}@long-domain-name-for-stress.example`,
	);
	const held: Record<string, unknown> = {};
	for (const width of FLOOR_WIDTHS) {
		await page.viewport(width, 720);
		open({ members: { members: [MURIEL, long], total: 2 } });
		await expect.element(table()).toBeVisible();
		held[`members ${width}`] = nameFloor(table().element(), "Name");
		dispose?.();
		const root = document.body.appendChild(document.createElement("div"));
		dispose = render(
			() => (
				<KeyTable
					status="active"
					label="Active keys"
					empty="No keys"
					keys={[
						{
							uuid: "k",
							name: STRESS.name,
							creation: "2026-09-01T00:00:00Z",
							expiration: "2026-12-01T00:00:00Z",
						},
					]}
					made={new Set()}
					onRevoke={() => {}}
				/>
			),
			root,
		);
		const keys = page.getByRole("table", { name: "Active keys" });
		await expect.element(keys).toBeVisible();
		held[`keys ${width}`] = nameFloor(keys.element(), "Name");
		dispose();
		dispose = undefined;
		root.remove();
	}
	const folded = { name: "folded", sideways: false };
	const fine = { name: "held", sideways: false };
	expect(held).toEqual({
		"members 320": folded,
		"members 768": fine,
		"members 1024": fine,
		"members 1440": fine,
		"keys 320": folded,
		"keys 768": fine,
		"keys 1024": fine,
		"keys 1440": fine,
	});
});

// Kills a members name cell that takes the room the email's share leaves, as
// one without `ui-table-name` does.
test("at 600 px the members name and email share the room Remove leaves", async () => {
	await page.viewport(1440, 720);
	open({
		members: {
			members: [
				MURIEL,
				member(
					"x",
					STRESS.name,
					`${"m".repeat(64)}@long-domain-name-for-stress.example`,
				),
			],
			total: 2,
		},
	});
	await expect.element(table()).toBeVisible();
	await document.fonts.ready;
	sizeTo(table().element(), 600);
	const row = table()
		.element()
		.querySelector("tbody tr:last-child") as HTMLElement;
	const width = (fold: string) =>
		(
			row.querySelector(`td[data-fold="${fold}"]`) as HTMLElement
		).getBoundingClientRect().width;
	expect(Math.round(width("l1"))).toBe(Math.round(width("l2")));
	expect(width("l1")).toBeGreaterThan(width("act"));
	document.body.style.width = "";
});

// Kills a members or key list that folds a pixel late or early, and a Revoke
// kept beside a folded key's name where it leaves the name under 16
// characters.
test("the members and key lists fold at their own widths, and Revoke takes a line of its own", async () => {
	await page.viewport(1440, 720);
	open({
		members: {
			members: [
				MURIEL,
				member(
					"x",
					STRESS.name,
					`${"m".repeat(64)}@long-domain-name-for-stress.example`,
				),
			],
			total: 2,
		},
	});
	await expect.element(table()).toBeVisible();
	await document.fonts.ready;
	expectFoldAt(table().element(), "Name", 369);
	dispose?.();
	const root = document.body.appendChild(document.createElement("div"));
	dispose = render(
		() => (
			<KeyTable
				status="active"
				label="Active keys"
				empty="No keys"
				keys={[
					{
						uuid: "k",
						name: STRESS.name,
						creation: "2026-09-01T00:00:00Z",
						expiration: "2026-12-01T00:00:00Z",
					},
				]}
				made={new Set()}
				onRevoke={() => {}}
			/>
		),
		root,
	);
	const keys = page.getByRole("table", { name: "Active keys" });
	await expect.element(keys).toBeVisible();
	expectFoldAt(keys.element(), "Name", 407);
	expectLinesAt(keys.element(), 352, 3);
	// On a phone's page the name holds WebKit's 16 characters (131.2px at 13px), wider than Chromium's.
	sizeTo(keys.element(), 290);
	const name = keys
		.element()
		.querySelector('td[data-fold="l1"]') as HTMLElement;
	expect(name.clientWidth).toBeGreaterThanOrEqual(131.2);
	const box = name.getBoundingClientRect();
	const beside = [...(name.parentElement as HTMLElement).children]
		.filter((cell) => cell !== name && cell.getClientRects().length > 0)
		.map((cell) => cell.getBoundingClientRect())
		.filter(
			(other) =>
				other.left < box.right &&
				other.right > box.left &&
				other.top < box.bottom &&
				other.bottom > box.top,
		);
	expect(beside).toEqual([]);
	const third = ["n1", "act"].map(
		(slot) =>
			(name.parentElement as HTMLElement)
				.querySelector(`td[data-fold="${slot}"]`)
				?.getBoundingClientRect().top,
	);
	expect(third[0]).toBeCloseTo(third[1] ?? 0, 0);
	expect(third[0]).toBeGreaterThan(box.bottom);
	document.body.style.width = "";
});
