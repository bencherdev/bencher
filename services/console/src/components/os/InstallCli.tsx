import {
	createResource,
	createSignal,
	Match,
	Switch,
	type Accessor,
} from "solid-js";
import { getOperatingSystem, OperatingSystem } from "./operating_system";

const InstallCli = (props: { linux; macos; windows; other }) => {
	const [os, setOs] = createSignal<undefined | OperatingSystem>();

	const [_operating_system] = createResource(async () => {
		const os = await getOperatingSystem();
		setOs(os);
	});

	return (
		<>
			<OperatingSystemButtons os={os} handleOs={setOs} />
			<Switch>
				<Match when={os() === OperatingSystem.Linux}>{props.linux}</Match>
				<Match when={os() === OperatingSystem.MacOS}>{props.macos}</Match>
				<Match when={os() === OperatingSystem.Windows}>{props.windows}</Match>
				<Match when={os() === OperatingSystem.Other}>{props.other}</Match>
			</Switch>
		</>
	);
};

const OperatingSystemButtons = (props: {
	os: Accessor<undefined | OperatingSystem>;
	handleOs: (os: OperatingSystem) => void;
}) => {
	return (
		<div class="field has-addons is-fullwidth" style={{ overflow: "auto" }}>
			<p class="control">
				<OperatingSystemButton
					name="Linux"
					icon="fab fa-linux"
					operating_system={OperatingSystem.Linux}
					os={props.os}
					handleOs={props.handleOs}
				/>
			</p>
			<p class="control">
				<OperatingSystemButton
					name="MacOS"
					icon="fab fa-apple"
					operating_system={OperatingSystem.MacOS}
					os={props.os}
					handleOs={props.handleOs}
				/>
			</p>
			<p class="control">
				<OperatingSystemButton
					name="Windows"
					icon="fab fa-windows"
					operating_system={OperatingSystem.Windows}
					os={props.os}
					handleOs={props.handleOs}
				/>
			</p>
			<p class="control">
				<OperatingSystemButton
					name="Other"
					icon="fas fa-server"
					operating_system={OperatingSystem.Other}
					os={props.os}
					handleOs={props.handleOs}
				/>
			</p>
		</div>
	);
};

const OperatingSystemButton = (props: {
	name: string;
	icon: string;
	operating_system: OperatingSystem;
	os: Accessor<undefined | OperatingSystem>;
	handleOs: (os: OperatingSystem) => void;
}) => {
	return (
		<button
			type="button"
			class={`button${props.operating_system === props.os() ? " is-active" : ""}`}
			onMouseDown={(e) => {
				e.preventDefault();
				props.handleOs(props.operating_system);
			}}
		>
			<span class="icon is-small">
				<i class={props.icon} />
			</span>
			<span>{props.name}</span>
		</button>
	);
};

export default InstallCli;
