import { AlertStatus, type JsonAlert } from "../../../../types/bencher";
import { BRANCH_ICON } from "../../../../config/project/branches";
import { TESTBED_ICON } from "../../../../config/project/testbeds";
import { MEASURE_ICON } from "../../../../config/project/measures";
import DimensionLabel from "./DimensionLabel";
import { MODEL_TEST_ICON } from "../../../field/kinds/Model";
import { Match, Switch } from "solid-js";

export const AlertRow = (props: { alert: JsonAlert }) => {
	return (
		<div>
			<Switch>
				<Match when={props.alert?.status === AlertStatus.Active}>
					<span class="icon-text">
						<span class="icon has-text-primary">
							<i class="far fa-bell" />
						</span>
						<span>{fmtAlertStatus(props.alert?.status)}</span>
					</span>
				</Match>
				<Match
					when={
						props.alert?.status === AlertStatus.Dismissed ||
						props.alert?.status === AlertStatus.Silenced
					}
				>
					<span class="icon-text">
						<span class="icon">
							<i class="far fa-bell-slash" />
						</span>
						<span>{fmtAlertStatus(props.alert?.status)}</span>
					</span>
				</Match>
			</Switch>
			<DimensionLabel
				icon={BRANCH_ICON}
				name={props.alert?.threshold?.branch?.name}
			/>
			<DimensionLabel
				icon={TESTBED_ICON}
				name={props.alert?.threshold?.testbed?.name}
			/>
			<DimensionLabel
				icon={MEASURE_ICON}
				name={props.alert?.threshold?.measure?.name}
			/>
			<DimensionLabel
				icon={MODEL_TEST_ICON}
				name={props.alert?.threshold?.model?.test ?? "No model"}
			/>
		</div>
	);
};

export const fmtAlertStatus = (status: AlertStatus) => {
	switch (status) {
		case AlertStatus.Active:
			return "Active";
		case AlertStatus.Dismissed:
			return "Dismissed";
		case AlertStatus.Silenced:
			return "Silenced";
		default:
			return "Unknown";
	}
};

export default AlertRow;
