import { themeSignal } from "../../navbar/theme/util";
import OnboardStepsInner, { type Props } from "./OnboardStepsInner";

const OnboardSteps = (props: Props) => {
	const theme = themeSignal;

	return <OnboardStepsInner {...props} theme={theme()} />;
};

export default OnboardSteps;
