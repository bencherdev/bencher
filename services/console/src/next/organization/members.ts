import type { QueryClient } from "@tanstack/solid-query";
import type { JsonMember } from "../../types/bencher";
import type { Api } from "../api";

const path = (organization: string) =>
	`/v0/organizations/${encodeURIComponent(organization)}/members`;

export interface MemberPage {
	members: JsonMember[];
	/** Every member, which the page counts. */
	total: number;
}

// The API's largest page, in name order.
const PER_PAGE = 255;

export const membersQuery = (api: Api, organization: string) => ({
	queryKey: ["organization", organization, "members"],
	queryFn: async ({ signal }: { signal: AbortSignal }): Promise<MemberPage> => {
		const { data, headers } = await api.get<JsonMember[]>(
			`${path(organization)}?per_page=${PER_PAGE}`,
			signal,
		);
		return {
			members: data,
			total: Number(headers.get("X-Total-Count") ?? data.length),
		};
	},
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

/** Remove the member, then drop their row from the cached list. */
export const removeMember = async (
	api: Api,
	client: QueryClient,
	organization: string,
	uuid: string,
) => {
	await api.send("DELETE", `${path(organization)}/${uuid}`);
	client.setQueryData<MemberPage>(
		membersQuery(api, organization).queryKey,
		(page) =>
			page && {
				members: page.members.filter((member) => member.uuid !== uuid),
				total: page.total - 1,
			},
	);
};

/** The API refuses an address without a name before its `@`. */
export const sendable = (email: string) => email.trim().indexOf("@") > 0;

export const memberCount = (count: number) =>
	`${count} ${count === 1 ? "member" : "members"}`;

const addedFormat = new Intl.DateTimeFormat("en-US", {
	month: "short",
	day: "numeric",
	year: "numeric",
});

/** "Added Mar 2, 2026". */
export const added = (created: string) =>
	`Added ${addedFormat.format(Date.parse(created))}`;
