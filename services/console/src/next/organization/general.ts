import { ApiError } from "../api";
import { formatCount } from "../plot/count";

/** General's form as the reader has it. */
export interface Draft {
	name: string;
	slug: string;
}

export type Field = keyof Draft;

export const draftOf = ({ name, slug }: Draft): Draft => ({ name, slug });

/** What the reader changed, or nothing when the form matches the organization. */
export const patchOf = (
	saved: Draft,
	draft: Draft,
): Partial<Draft> | undefined => {
	const patch: Partial<Draft> = {};
	const name = draft.name.trim();
	if (name !== saved.name) {
		patch.name = name;
	}
	if (draft.slug !== saved.slug) {
		patch.slug = draft.slug;
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
});

// The most projects the clause names; it counts the rest.
const NAMED = 5;

/** The delete confirmation's clause naming the projects deleted with the organization. */
export const goneWith = (projects: readonly string[]) =>
	projects.length
		? `${new Intl.ListFormat("en").format(
				projects.length > NAMED
					? [
							...projects.slice(0, NAMED),
							`${formatCount(projects.length - NAMED)} more`,
						]
					: projects,
			)} ${projects.length === 1 ? "goes" : "go"} with it.`
		: undefined;

const CONFLICT = 409;

/** Why a save did not happen, in the API's words when it gave them. */
export const saveFailure = (error: unknown, patch: Partial<Draft>) =>
	error instanceof ApiError &&
	error.status === CONFLICT &&
	patch.slug !== undefined
		? "Another organization has this slug"
		: failureOf(error, "Bencher did not save the changes");

/** What did not happen, with what the API said when it refused. */
export const failureOf = (error: unknown, failed: string) => {
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
