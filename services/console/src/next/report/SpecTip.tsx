import Tooltip from "@bencherdev/ui/Tooltip";
import type { JsonConsoleSpec } from "../../types/bencher";

const GIB = 1024 ** 3;

/** A runner report's spec by name, with what it ran on in a tooltip. */
const SpecTip = (props: { spec: JsonConsoleSpec }) => (
	<Tooltip
		tip={
			<span class="rn-spectip">
				<span>OS</span>
				<b>{props.spec.os}</b>
				<span>Architecture</span>
				<b>{props.spec.architecture}</b>
				<span>CPU</span>
				<b>{props.spec.cpu}</b>
				<span>Memory</span>
				<b>{Math.round((props.spec.memory / GIB) * 10) / 10} GiB</b>
			</span>
		}
	>
		<button type="button" class="rn-specname">
			{props.spec.name}
		</button>
	</Tooltip>
);

export default SpecTip;
