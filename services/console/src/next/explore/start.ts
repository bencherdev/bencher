import type { JsonAlert } from "../../types/bencher";
import type { SelectedLine } from "../query/selection";

// A threshold with no metric checks the conventional name.
const VALUE = "value";

/** The line an alert fired on, as Explore opens it. */
export const alertLine = ({
	threshold,
	benchmark,
	variant,
}: JsonAlert): SelectedLine => ({
	branch: { uuid: threshold.branch.uuid },
	testbed: { uuid: threshold.testbed.uuid },
	benchmark: benchmark.uuid,
	parameters: variant.parameters,
	measure: threshold.measure.uuid,
	metric: threshold.metric ?? VALUE,
	alerting: true,
});
