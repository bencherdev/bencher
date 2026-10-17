import type { JsonUser } from "../../types/bencher";
import { type Api, ApiError } from "../api";

export const userQuery = (api: Api, user: string) => ({
	queryKey: ["account", "user", user],
	queryFn: async ({ signal }: { signal: AbortSignal }) =>
		(await api.get<JsonUser>(`/v0/users/${encodeURIComponent(user)}`, signal))
			.data,
});

/** How many legacy tokens the user has not revoked: the tokens list's total. */
export const legacyTokensQuery = (api: Api, user: string) => ({
	queryKey: ["account", "tokens", user],
	queryFn: async ({ signal }: { signal: AbortSignal }) => {
		const { headers } = await api.get<unknown>(
			`/v0/users/${encodeURIComponent(user)}/tokens?per_page=1`,
			signal,
		);
		return Number(headers.get("X-Total-Count")) || 0;
	},
});

export const legacyTokens = (count: number) =>
	count === 1 ? "1 legacy token" : `${count} legacy tokens`;

/** Profile's form as the reader has it. */
export interface Draft {
	name: string;
	slug: string;
	email: string;
}

export type Field = keyof Draft;

export const draftOf = ({ name, slug, email }: JsonUser): Draft => ({
	name,
	slug,
	email,
});

/** What the reader changed, or nothing when the form matches the account. */
export const patchOf = (
	saved: Draft,
	draft: Draft,
): Partial<Draft> | undefined => {
	const patch: Partial<Draft> = {};
	const name = draft.name.trim();
	const email = draft.email.trim();
	if (name !== saved.name) {
		patch.name = name;
	}
	if (draft.slug !== saved.slug) {
		patch.slug = draft.slug;
	}
	if (email !== saved.email) {
		patch.email = email;
	}
	return Object.keys(patch).length > 0 ? patch : undefined;
};

const SLUG = /^[a-z0-9]+(?:-[a-z0-9]+)*$/;

/** What the API would surely refuse in each field; it says why for the rest. */
export const problemsOf = (
	draft: Draft,
): Record<Field, string | undefined> => ({
	name: draft.name.trim() ? undefined : "Required",
	slug: SLUG.test(draft.slug)
		? undefined
		: "Lowercase letters, numbers, and single dashes",
	email: draft.email.trim() ? undefined : "Required",
});

const FAILED = "Bencher did not save the changes";

/** Why a save did not happen, in the API's words when it gave them. */
export const failureOf = (error: unknown, failed = FAILED) => {
	if (!(error instanceof ApiError) || error.status === undefined) {
		return failed;
	}
	try {
		const message = JSON.parse(error.message)?.message;
		if (typeof message === "string" && message) {
			return `${failed}: ${message}`;
		}
	} catch {
		// Not the API's JSON; its status says enough.
	}
	return `${failed}: the API answered ${error.status}`;
};
