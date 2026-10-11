import type { Guard } from "../plot/format";

interface Delta {
	text: string;
	arrow: "↑" | "↓" | null;
	word: "worse" | "better" | null;
}

// A change under one percent names no direction.
const NOISE = 0.01;

export const guardOf = (model?: {
	lower_boundary?: number | null;
	upper_boundary?: number | null;
}): Guard | undefined => {
	const lower = model?.lower_boundary != null;
	const upper = model?.upper_boundary != null;
	if (lower && upper) {
		return "both";
	}
	return upper ? "upper" : lower ? "lower" : undefined;
};

export const deltaOf = (
	value: number,
	baseline: number | null | undefined,
	guard: Guard | undefined,
): Delta | null => {
	if (!guard || baseline == null || baseline === 0) {
		return null;
	}
	// Over the baseline's size, so a negative baseline keeps the sign of the move, as the API's delta sort does.
	const delta = (value - baseline) / Math.abs(baseline);
	const percent = Math.abs(delta * 100).toFixed(1);
	const text = `${delta < 0 && percent !== "0.0" ? "-" : "+"}${percent}%`;
	if (Math.abs(delta) < NOISE) {
		return { text, arrow: null, word: null };
	}
	const worse = guard === "both" || (guard === "upper" ? delta > 0 : delta < 0);
	return {
		text,
		arrow: delta > 0 ? "↑" : "↓",
		word: worse ? "worse" : "better",
	};
};
