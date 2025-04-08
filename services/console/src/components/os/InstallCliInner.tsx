import { createResource, createSignal, Match, Switch } from "solid-js";
import { getOperatingSystem, OperatingSystem } from "./operating_system";
import { OperatingSystemButtons } from "./InstallCliFallback";

const InstallCliInner = (props: {
	centered: undefined | boolean;
	linux;
	macos;
	windows;
	other;
}) => {
	const [os, setOs] = createSignal<undefined | OperatingSystem>();

	const [_operating_system] = createResource(async () => {
		const os = await getOperatingSystem();
		setOs(os);
	});

	return (
		<>
			<OperatingSystemButtons
				os={os}
				handleOs={setOs}
				centered={props.centered}
			/>
			<Switch>
				<Match when={os() === OperatingSystem.Linux}>{props.linux}</Match>
				<Match when={os() === OperatingSystem.MacOS}>{props.macos}</Match>
				<Match when={os() === OperatingSystem.Windows}>{props.windows}</Match>
				<Match when={os() === OperatingSystem.Other}>{props.other}</Match>
			</Switch>
		</>
	);
};

export default InstallCliInner;
