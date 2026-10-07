type Column = readonly (number | null)[] | undefined;

/** A band side with no limit closes on the plot's edge. */
export const EDGE = "edge";

interface BandPoint {
	index: number;
	top: number | typeof EDGE;
	bottom: number | typeof EDGE;
}

export interface BandGeometry {
	/** Runs of the shaded area: below an upper limit, above a lower one, or between both. */
	areas: BandPoint[][];
	/** Runs of indices that carry each limit, for its dashed line. */
	lower: number[][];
	upper: number[][];
}

/** A line's boundary in index space, walking the line's own points so that only a point with no limit breaks it. */
export const bandGeometry = (
	y: readonly (number | null)[],
	lower: Column,
	upper: Column,
): BandGeometry => {
	const geometry: BandGeometry = { areas: [], lower: [], upper: [] };
	if (!lower && !upper) {
		return geometry;
	}
	let area: BandPoint[] | undefined;
	let lowerRun: number[] | undefined;
	let upperRun: number[] | undefined;
	for (let index = 0; index < y.length; index++) {
		if (y[index] == null) {
			continue;
		}
		const low = lower?.[index] ?? null;
		const high = upper?.[index] ?? null;
		if (low === null && high === null) {
			area = undefined;
		} else {
			if (!area) {
				area = [];
				geometry.areas.push(area);
			}
			area.push({ index, top: high ?? EDGE, bottom: low ?? EDGE });
		}
		lowerRun = extend(geometry.lower, lowerRun, low, index);
		upperRun = extend(geometry.upper, upperRun, high, index);
	}
	return geometry;
};

const extend = (
	runs: number[][],
	run: number[] | undefined,
	limit: number | null,
	index: number,
): number[] | undefined => {
	if (limit === null) {
		return undefined;
	}
	if (run) {
		run.push(index);
		return run;
	}
	const next = [index];
	runs.push(next);
	return next;
};
