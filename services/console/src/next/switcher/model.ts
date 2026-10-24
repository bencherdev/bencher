import type { JsonConsoleUser } from "../../types/bencher";
import { projectAt } from "../home";
import type { BmfVersion } from "../memory";
import { projectPath } from "../paths";
import { relativeTime } from "../reports/format";

export interface SwitcherRow {
	slug: string;
	name: string;
	/** Named only for a reader of more than one organization. */
	organization: string | undefined;
	/** Searched whether or not the row names it. */
	organizationName: string;
	href: string;
	version: BmfVersion;
	/** How long ago it last ran, if it ever has. */
	age: string | undefined;
	alerts: number;
	current: boolean;
}

/** A text split around where a search matched it: before, the match, after. */
export type Split = [string, string, string];

/** Every project of the reader's, those `recent` names first in its order, the rest by newest run. */
export const switcherRows = (
	user: JsonConsoleUser,
	recent: readonly string[],
	current: string,
	now: number,
): SwitcherRow[] => {
	const organizations = new Map(
		user.organizations.map(({ uuid, name }) => [uuid, name]),
	);
	const many = organizations.size > 1;
	const rank = (slug: string) => {
		const index = recent.indexOf(slug);
		return index < 0 ? recent.length : index;
	};
	return [...user.projects]
		.sort(
			(a, b) =>
				rank(a.slug) - rank(b.slug) ||
				(b.last_report ?? 0) - (a.last_report ?? 0) ||
				a.name.localeCompare(b.name),
		)
		.map((project) => {
			const organizationName = organizations.get(project.organization) ?? "";
			return {
				slug: project.slug,
				name: project.name,
				organization: many ? organizationName : undefined,
				organizationName,
				href:
					projectAt(project.slug, project.bmf_version) ??
					projectPath(project.slug),
				version: project.bmf_version === 0 ? 0 : 1,
				age: project.last_report
					? relativeTime(project.last_report, now)
					: undefined,
				alerts: project.active_alerts,
				current: project.slug === current,
			};
		});
};

/** The rows whose name or organization holds `query` in any case. */
export const searchRows = (rows: readonly SwitcherRow[], query: string) => {
	const needle = query.trim().toLowerCase();
	return rows.flatMap((row) => {
		const name = split(row.name, needle);
		// The organization is marked only when the name did not match.
		const organization = split(row.organizationName, name[1] ? "" : needle);
		return !needle || name[1] || organization[1]
			? [
					{
						row,
						name,
						organization:
							row.organization === undefined ? undefined : organization,
					},
				]
			: [];
	});
};

const split = (text: string, needle: string): Split => {
	const at = needle ? text.toLowerCase().indexOf(needle) : -1;
	return at < 0
		? [text, "", ""]
		: [
				text.slice(0, at),
				text.slice(at, at + needle.length),
				text.slice(at + needle.length),
			];
};
