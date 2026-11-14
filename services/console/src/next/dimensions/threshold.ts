import type { JsonThreshold } from "../../types/bencher";
import { parameterTag } from "../tag";

/**
 * A threshold link's accessible name: the text the link shows, then which
 * threshold it opens, down to its parameter sets as the card writes them and a
 * metric other than `value`, since one measure can carry several thresholds.
 */
export const thresholdLinkName = (shown: string, threshold: JsonThreshold) =>
	[
		`${shown}, threshold for ${threshold.measure.name} on ${threshold.branch.name}`,
		threshold.testbed.name,
		...(threshold.parameters?.length
			? [
					threshold.parameters
						.map((set) =>
							Object.entries(set)
								.sort(([a], [b]) => (a < b ? -1 : 1))
								.map(([key, value]) => parameterTag(key, value))
								.join(" "),
						)
						.join(" or "),
				]
			: []),
		...(threshold.metric && threshold.metric !== "value"
			? [`metric ${threshold.metric}`]
			: []),
	].join(", ");
