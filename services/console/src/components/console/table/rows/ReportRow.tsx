import { Show } from "solid-js";
import { fmtDateTime } from "../../../../config/util";
import type { JsonReport } from "../../../../types/bencher";
import { ALERT_ICON, ALERT_OFF_ICON } from "../../../../config/project/alerts";
import { BRANCH_ICON } from "../../../../config/project/branches";
import { TESTBED_ICON } from "../../../../config/project/testbeds";
import { BENCHMARK_ICON } from "../../../../config/project/benchmarks";
import { MEASURE_ICON } from "../../../../config/project/measures";
import DimensionLabel, { ADAPTER_ICON } from "./DimensionLabel";

export const ReportRow = (props: { report: JsonReport }) => {
	const benchmarkCount =
		props.report?.counts?.results?.map((counts) => counts?.benchmarks) ?? [];

	const totalAlerts = props.report?.counts?.alerts?.total ?? 0;
	const activeAlerts = props.report?.counts?.alerts?.active ?? 0;

	return (
		<div>
			<small style="word-break: break-word;">
				{fmtDateTime(props.report?.start_time)}
			</small>
			<Show when={totalAlerts}>
				<DimensionLabel
					icon={activeAlerts === 0 ? ALERT_OFF_ICON : ALERT_ICON}
					iconClass={activeAlerts === 0 ? "" : " has-text-primary"}
					name={(() => {
						const active =
							activeAlerts === 0 || activeAlerts === totalAlerts
								? ""
								: ` (${activeAlerts} active | ${totalAlerts - activeAlerts} inactive)`;
						return `${totalAlerts} ${totalAlerts === 1 ? "alert" : "alerts"}${active}`;
					})()}
				/>
			</Show>
			<DimensionLabel icon={BRANCH_ICON} name={props.report?.branch?.name} />
			<DimensionLabel icon={TESTBED_ICON} name={props.report?.testbed?.name} />
			<DimensionLabel
				icon={BENCHMARK_ICON}
				name={(() => {
					if (benchmarkCount.length === 0) {
						return "0 benchmarks";
					}
					const plural =
						benchmarkCount.length > 1 ||
						benchmarkCount.some((count) => count > 1);
					return `${benchmarkCount.join(" + ")} benchmark${plural ? "s" : ""}`;
				})()}
			/>
			<DimensionLabel
				icon={MEASURE_ICON}
				name={(() => {
					const measureCount =
						props.report?.counts?.results?.map((counts) => counts?.measures) ??
						[];
					if (measureCount.length === 0) {
						return "0 measures";
					}
					const plural =
						measureCount.length > 1 || measureCount.some((count) => count > 1);
					return `${measureCount.join(" + ")} measure${plural ? "s" : ""}`;
				})()}
			/>
			<DimensionLabel
				icon={ADAPTER_ICON}
				name={props.report?.adapter ?? "Mystery"}
			/>
		</div>
	);
};

export default ReportRow;
