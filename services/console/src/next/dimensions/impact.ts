const REVIVES = "A run that reports it brings it back.";

/**
 * What archiving a dimension does, said before it happens. A benchmark owns
 * no thresholds, so it has no count: a threshold belongs to a branch, a
 * testbed, and a measure.
 */
export const archiveImpact = (name: string, thresholds: number | undefined) => {
	if (thresholds === undefined) {
		return `Archiving ${name} hides its lines. Its thresholds stay. ${REVIVES}`;
	}
	if (thresholds === 0) {
		return `Archiving ${name} hides its lines. No threshold checks it. ${REVIVES}`;
	}
	return `Archiving ${name} archives ${count(thresholds, "threshold")} and hides its lines. ${REVIVES}`;
};

export const VARIANT_IMPACT = `Archiving this variant hides its lines. Its thresholds stay. ${REVIVES}`;

/** What archiving did, once it is done. */
export const archivedNote = (name: string, thresholds: number | undefined) => {
	if (thresholds === undefined) {
		return `Archived ${name}. Its lines are hidden; its thresholds stay.`;
	}
	if (thresholds === 0) {
		return `Archived ${name}. Its lines are hidden.`;
	}
	return `Archived ${name} with its ${count(thresholds, "threshold")}. Its lines are hidden.`;
};

/**
 * What unarchiving did: `back` thresholds return with it, and `held` stay
 * archived because another of their dimensions is.
 */
export const unarchivedNote = (
	name: string,
	back: number | undefined,
	held: number,
) => {
	if (!(back || held)) {
		return `Unarchived ${name}. Its lines are back.`;
	}
	const returned = back
		? `Unarchived ${name} and ${count(back, "threshold")}.`
		: `Unarchived ${name}.`;
	if (held === 0) {
		return returned;
	}
	if (held === 1) {
		return `${returned} ${back ? "One" : "Its"} threshold stays archived: another of its dimensions is archived.`;
	}
	return `${returned} ${held} thresholds stay archived: another of their dimensions is archived.`;
};

const count = (n: number, one: string) => `${n} ${n === 1 ? one : `${one}s`}`;
