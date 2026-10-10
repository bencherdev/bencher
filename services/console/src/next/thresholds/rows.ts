import type {
	JsonConsoleThresholdBranch,
	JsonConsoleThresholdMeasure,
	JsonConsoleThresholdRow,
	JsonConsoleThresholdTestbed,
	JsonConsoleThresholds,
} from "../../types/bencher";

/** The metric a threshold checks when it names none. */
export const VALUE = "value";

/** A threshold of the list, resolved against the tables of the batch it came in. */
export interface ThresholdRow
	extends Omit<JsonConsoleThresholdRow, "branch" | "testbed" | "measure"> {
	branch: JsonConsoleThresholdBranch;
	testbed: JsonConsoleThresholdTestbed;
	measure: JsonConsoleThresholdMeasure;
	metric: string;
}

export const rowsOf = (batch: JsonConsoleThresholds): ThresholdRow[] =>
	batch.thresholds.flatMap((row) => {
		const branch = batch.branches[row.branch];
		const testbed = batch.testbeds[row.testbed];
		const measure = batch.measures[row.measure];
		if (!(branch && testbed && measure)) {
			return [];
		}
		return [{ ...row, branch, testbed, measure, metric: row.metric ?? VALUE }];
	});
