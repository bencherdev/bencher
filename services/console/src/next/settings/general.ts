import type { JsonProject, Visibility } from "../../types/bencher";
import { ApiError } from "../api";

/** The General form as the reader has it. */
export interface Draft {
	name: string;
	slug: string;
	url: string;
	visibility: Visibility;
}

/** A change to the project as the API takes it; a `null` URL removes it. */
export interface Patch {
	name?: string;
	slug?: string;
	url?: string | null;
	visibility?: Visibility;
}

export type Field = "name" | "slug" | "url";

export const draftOf = (project: JsonProject): Draft => ({
	name: project.name,
	slug: project.slug,
	url: project.url ?? "",
	visibility: project.visibility,
});

/** What the reader changed, or nothing when the form matches the project. */
export const patchOf = (saved: Draft, draft: Draft): Patch | undefined => {
	const patch: Patch = {};
	const name = draft.name.trim();
	const url = draft.url.trim();
	if (name !== saved.name) {
		patch.name = name;
	}
	if (draft.slug !== saved.slug) {
		patch.slug = draft.slug;
	}
	if (url !== saved.url) {
		patch.url = url || null;
	}
	if (draft.visibility !== saved.visibility) {
		patch.visibility = draft.visibility;
	}
	return Object.keys(patch).length > 0 ? patch : undefined;
};

// The API counts bytes, not letters.
const MAX_BYTES = 64;
const bytes = (text: string) => new TextEncoder().encode(text).length;
/** Over the API's 64 bytes for a name or a slug. */
export const tooLong = (text: string) => bytes(text) > MAX_BYTES;
export const TOO_LONG = "Too long: 64 bytes at most.";
const SLUG = /^[a-z0-9]+(?:-[a-z0-9]+)*$/;

/** What the API would refuse in each field, in words for the reader. */
export const problemsOf = (draft: Draft): Partial<Record<Field, string>> => {
	const problems: Partial<Record<Field, string>> = {};
	const name = draft.name.trim();
	if (!name) {
		problems.name = "A project needs a name.";
	} else if (tooLong(name)) {
		problems.name = TOO_LONG;
	}
	if (!SLUG.test(draft.slug) || tooLong(draft.slug)) {
		problems.slug =
			"Up to 64 lowercase letters and numbers, with single dashes between them.";
	}
	const url = draft.url.trim();
	if (url && !webUrl(url)) {
		problems.url = "A full URL, starting with https://";
	}
	return problems;
};

/** `url` when it opens a web page; any other scheme, such as `javascript:`, is no link. */
export const webUrl = (url: string | undefined) => {
	const protocol = url && URL.canParse(url) ? new URL(url).protocol : "";
	return protocol === "https:" || protocol === "http:" ? url : undefined;
};

/** The project as it will be once the API applies `patch`. */
export const patched = (project: JsonProject, patch: Patch): JsonProject => {
	const { url, ...rest } = patch;
	const next: JsonProject = { ...project, ...rest };
	if (url === null) {
		delete next.url;
	} else if (url !== undefined) {
		next.url = url;
	}
	return next;
};

export interface Refusal {
	/** Where the reader reads it: under the field, or at the form's foot. */
	field: "visibility" | "slug" | "form";
	message: string;
}

const PAYMENT_REQUIRED = 402;
const CONFLICT = 409;

/** Why a save did not happen, placed where the reader can act on it. */
export const refusalOf = (error: unknown, patch: Patch): Refusal => {
	if (error instanceof ApiError) {
		if (error.status === PAYMENT_REQUIRED && patch.visibility !== undefined) {
			return {
				field: "visibility",
				message: "Bencher kept the project public: private needs a paid plan.",
			};
		}
		if (error.status === CONFLICT && patch.slug !== undefined) {
			return {
				field: "slug",
				message: "Another project already has this slug.",
			};
		}
	}
	return {
		field: "form",
		message: failureOf(
			error,
			"Bencher did not save the changes",
			"The Bencher API did not answer, so nothing was saved.",
		),
	};
};

/** What the API said when it refused, or `unanswered` when it never answered. */
export const failureOf = (
	error: unknown,
	refused: string,
	unanswered: string,
) =>
	error instanceof ApiError && error.status !== undefined
		? `${refused}: ${apiMessage(error)}`
		: unanswered;

const apiMessage = (error: ApiError) => {
	try {
		const message = JSON.parse(error.message)?.message;
		if (typeof message === "string" && message) {
			return message;
		}
	} catch {
		// Not the API's JSON; its status says enough.
	}
	return `the API answered ${error.status}.`;
};
