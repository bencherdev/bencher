import { createResource, Show } from "solid-js";
import { useBash } from "./operating_system";

const TerminalCommand = (props: { bash; powershell }) => {
	const [bash] = createResource(useBash);

	return (
		<div>
			<Show when={bash()} fallback={props.powershell}>
				{props.bash}
			</Show>
		</div>
	);
};

export default TerminalCommand;
