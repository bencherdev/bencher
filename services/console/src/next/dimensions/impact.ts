import { formatCount } from "../plot/count";

/** What archiving a dimension does, said before it happens; a benchmark or a variant owns no thresholds, so it has no count. */
export const archiveImpact = (thresholds: number | undefined) =>
	thresholds
		? `Hides its lines and archives ${count(thresholds, "threshold")}`
		: "Hides its lines";

/** What archiving did, once it is done, beside the row that names it. */
export const archivedNote = (thresholds: number | undefined) =>
	thresholds ? `Archived with ${count(thresholds, "threshold")}` : "Archived";

/**
 * What unarchiving did: `back` thresholds return with it, and `held` stay
 * archived because another of their dimensions is.
 */
export const unarchivedNote = (back: number | undefined, held: number) => {
	const returned = back
		? `Unarchived with ${count(back, "threshold")}`
		: "Unarchived";
	return held === 0
		? returned
		: `${returned} · ${count(held, "threshold")} still archived`;
};

const count = (n: number, one: string) =>
	`${formatCount(n)} ${n === 1 ? one : `${one}s`}`;
