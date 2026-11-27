import type { QueryClient } from "@tanstack/solid-query";
import type { JsonInvite, JsonMember } from "../../types/bencher";
import type { Api } from "../api";
import { createPaged } from "../list/paged";
import { dropRow, totalOf } from "../list/pages";
import { screenBatch } from "../list/rows";
import { formatCount } from "../plot/count";

const path = (organization: string) =>
	`/v0/organizations/${encodeURIComponent(organization)}/members`;

export const membersKey = (organization: string) => [
	"organization",
	organization,
	"members",
];

/** One page of the organization's members, in name order. */
export const membersPage =
	(api: Api, organization: string) =>
	async (page: number, perPage: number, signal: AbortSignal) => {
		const { data, headers } = await api.get<JsonMember[]>(
			`${path(organization)}?per_page=${perPage}&page=${page}`,
			signal,
		);
		return { rows: data, total: totalOf(headers, data) };
	};

/** The organization's members, a screen's worth at a time. */
export const memberList = (api: Api, organization: () => string) =>
	createPaged<JsonMember>({
		queryKey: () => membersKey(organization()),
		perPage: screenBatch(),
		fetchPage: (page, perPage, signal) =>
			membersPage(api, organization())(page, perPage, signal),
	});

/** Whether the reader may invite and remove members, as the API rules it. */
export interface Roles {
	invite: boolean;
	remove: boolean;
}

export const rolesQuery = (api: Api, organization: string) => ({
	queryKey: ["organization", organization, "roles"],
	queryFn: async ({ signal }: { signal: AbortSignal }): Promise<Roles> => {
		const allowed = async (permission: string) =>
			(
				await api.get<{ allowed: boolean }>(
					`/v0/organizations/${encodeURIComponent(organization)}/allowed/${permission}`,
					signal,
				)
			).data.allowed;
		const [invite, remove] = await Promise.all([
			allowed("create_role"),
			allowed("delete_role"),
		]);
		return { invite, remove };
	},
});

export const invite = (api: Api, organization: string, email: string) =>
	api.send("POST", path(organization), { email, role: "leader" });

const invitesPath = (organization: string) =>
	`/v0/organizations/${encodeURIComponent(organization)}/invites`;

/** The most invitations the API answers at once, as many as a Leader sends. */
const INVITES_PER_PAGE = 255;

/** The organization's pending invitations, newest first, and how many there are. */
export const invitesQuery = (api: Api, organization: string) => ({
	queryKey: ["organization", organization, "invites"],
	queryFn: async ({ signal }: { signal: AbortSignal }) => {
		const { data, headers } = await api.get<JsonInvite[]>(
			`${invitesPath(organization)}?per_page=${INVITES_PER_PAGE}`,
			signal,
		);
		return { invites: data, total: totalOf(headers, data) };
	},
});

/** Email the invitation again with a new link, which lives 7 more days. */
export const resendInvite = (api: Api, organization: string, uuid: string) =>
	api.send<JsonInvite>(
		"POST",
		`${invitesPath(organization)}/${encodeURIComponent(uuid)}/resend`,
	);

/** End the invitation, so its link no longer works. */
export const revokeInvite = (api: Api, organization: string, uuid: string) =>
	api.send(
		"DELETE",
		`${invitesPath(organization)}/${encodeURIComponent(uuid)}`,
	);

/** Remove the member, then drop their row from the cached list. */
export const removeMember = async (
	api: Api,
	client: QueryClient,
	organization: string,
	uuid: string,
) => {
	await api.send("DELETE", `${path(organization)}/${uuid}`);
	dropRow<JsonMember>(client, membersKey(organization), uuid);
};

/** The API refuses an address without a name before its `@`. */
export const sendable = (email: string) => email.trim().indexOf("@") > 0;

export const memberCount = (count: number) =>
	`${formatCount(count)} ${count === 1 ? "member" : "members"}`;

const addedFormat = new Intl.DateTimeFormat("en-US", {
	month: "short",
	day: "numeric",
	year: "numeric",
});

/** "Added Mar 2, 2026". */
export const added = (created: string) =>
	`Added ${addedFormat.format(Date.parse(created))}`;

/** When an invitation stops working, "Nov 5, 2026", or when it stopped, "Expired Oct 27, 2026". */
export const expires = (expiration: string, now: number) => {
	const at = Date.parse(expiration);
	const date = addedFormat.format(at);
	return at > now
		? { text: date, expired: false }
		: { text: `Expired ${date}`, expired: true };
};
