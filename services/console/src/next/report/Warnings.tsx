import Table from "@bencherdev/ui/Table";
import { For, type JSX } from "solid-js";
import type {
	JsonReportWarning,
	ReportWarningAction,
	ReportWarningResource,
} from "../../types/bencher";
import { formatCount } from "../plot/count";

/** Report warnings under the Resource, Action, and Count columns. */
export const WarningsTable = (props: {
	label: string;
	children: JSX.Element;
}) => (
	<Table aria-label={props.label}>
		<thead>
			<tr>
				<th scope="col">Resource</th>
				<th scope="col" style="width:30%">
					Action
				</th>
				<th scope="col" data-align="end" style="width:12%">
					Count
				</th>
			</tr>
		</thead>
		<tbody>{props.children}</tbody>
	</Table>
);

/** One row per warning, in the order the API gives them. */
export const WarningRows = (props: {
	warnings: readonly JsonReportWarning[];
}) => (
	<For each={props.warnings}>
		{(warning) => (
			<tr>
				<td>{RESOURCES[warning.resource]}</td>
				<td>{ACTIONS[warning.action]}</td>
				<td data-align="end">{formatCount(warning.count)}</td>
			</tr>
		)}
	</For>
);

/** How many resources the warnings count, which is what a count of warnings reads. */
export const warningsTotal = (warnings: readonly JsonReportWarning[]) =>
	warnings.reduce((sum, warning) => sum + warning.count, 0);

// Keyed by the enums' wire values, so the page loads no enum object.
const RESOURCES: Record<`${ReportWarningResource}`, string> = {
	benchmark: "Benchmark",
	variant: "Variant",
	measure: "Measure",
	metric: "Metric",
	threshold: "Threshold",
	adapter: "Adapter",
	average: "Average",
};

const ACTIONS: Record<`${ReportWarningAction}`, string> = {
	skip: "Skipped",
	ignore: "Ignored",
};
