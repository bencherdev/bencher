import type { Accessor } from "solid-js";
import { OperatingSystem } from "./operating_system";

const InstallCliFallback = (props: { centered: undefined | boolean; children }) => {
	return (
		<>
			<OperatingSystemButtons centered={props.centered} />
			{props.children}
		</>
	);
};


export const OperatingSystemButtons = (props: {
	centered: undefined | boolean;
	os?: Accessor<undefined | OperatingSystem>;
	handleOs?: (os: OperatingSystem) => void;
}) => {
	return (
		<div class={`field has-addons${props.centered ? " has-addons-centered": ""} is-fullwidth`} style={{ overflow: "auto" }}>
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

export const OperatingSystemButton = (props: {
	name: string;
	icon: string;
	operating_system: OperatingSystem;
	os: undefined | Accessor<undefined | OperatingSystem>;
	handleOs: undefined | ((os: OperatingSystem) => void);
}) => {
	return (
		<button
			type="button"
			class={`button${props.operating_system === props.os?.() ? " is-active" : ""}`}
			onMouseDown={(e) => {
				e.preventDefault();
				props.handleOs?.(props.operating_system);
			}}
		>
			<span class="icon is-small">
				<i class={props.icon} />
			</span>
			<span>{props.name}</span>
		</button>
	);
};

export default InstallCliFallback;
