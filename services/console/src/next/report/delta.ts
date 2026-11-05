import type { Guard } from "../plot/format";

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
