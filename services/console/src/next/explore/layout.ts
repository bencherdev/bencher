import type { Layout } from "../query/query";

/** The most lines a plot draws, as the plot query caps them. */
export const LINE_CAP = 64;
/** Where the line count starts to show, an eighth below the cap. */
const NEAR_CAP = 56;

export interface MeasuresLayout {
	readonly layout: Layout;
	/** Whether the layout control shows, and whether the reader may pick. */
	readonly control: "none" | "choice" | "forced";
}

/** Dual axis by default at two measures, stacked always at three or more. */
export const measuresLayout = (
	measures: number,
	chosen: Layout,
): MeasuresLayout => {
	if (measures >= 3) {
		return { layout: "stacked", control: "forced" };
	}
	return { layout: chosen, control: measures === 2 ? "choice" : "none" };
};

/** Whether the query's line count shows, from the number of lines its boxes name. */
export const lineCapWarning = (total: number): "near" | "over" | undefined => {
	if (total > LINE_CAP) {
		return "over";
	}
	return total >= NEAR_CAP ? "near" : undefined;
};
