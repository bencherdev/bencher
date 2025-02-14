import { themeSignal } from "../../navbar/theme/util";
import OnboardStepsInner, { type Props } from "./OnboardStepsInner";

const OnboardSteps = (props: Props) => {
	return <OnboardStepsInner {...props} theme={themeSignal} />;
};

export default OnboardSteps;
