import type { JsonThreshold } from "../../../../types/bencher";
import { BRANCH_ICON } from "../../../../config/project/branches";
import { TESTBED_ICON } from "../../../../config/project/testbeds";
import { MEASURE_ICON } from "../../../../config/project/measures";
import DimensionLabel from "./DimensionLabel";
import { MODEL_TEST_ICON } from "../../../field/kinds/Model";

export const ThresholdRow = (props: { threshold: JsonThreshold }) => {
	return (
		<div>
			<DimensionLabel icon={BRANCH_ICON} name={props.threshold?.branch?.name} />
			<DimensionLabel
				icon={TESTBED_ICON}
				name={props.threshold?.testbed?.name}
			/>
			<DimensionLabel
				icon={MEASURE_ICON}
				name={props.threshold?.measure?.name}
			/>
			<DimensionLabel
				icon={MODEL_TEST_ICON}
				name={props.threshold?.model?.test ?? "No model"}
			/>
		</div>
	);
};

export default ThresholdRow;
