import { formatCount } from "../plot/count";

/** What archiving a dimension does, said before it happens; a benchmark or a variant owns no thresholds, so it has no count. */
export const archiveImpact = (thresholds: number | undefined) =>
	thresholds
		? `Hides its lines and archives ${count(thresholds, "threshold")}`
		: "Hides its lines";

/** What archiving did, once it is done. */
export const archivedNote = (name: string, thresholds: number | undefined) =>
	thresholds
		? `Archived ${name} and ${count(thresholds, "threshold")}`
		: `Archived ${name}`;

/**
 * What unarchiving did: `back` thresholds return with it, and `held` stay
 * archived because another of their dimensions is.
 */
export const unarchivedNote = (
	name: string,
	back: number | undefined,
	held: number,
) => {
	const returned = back
		? `Unarchived ${name} and ${count(back, "threshold")}`
		: `Unarchived ${name}`;
	return held === 0
		? returned
		: `${returned} · ${count(held, "threshold")} still archived`;
};

const count = (n: number, one: string) =>
	`${formatCount(n)} ${n === 1 ? one : `${one}s`}`;
