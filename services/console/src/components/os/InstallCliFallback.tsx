import { OperatingSystem } from "./operating_system";

const InstallCliFallback = (props: { children }) => {
	return (
		<>
			<OperatingSystemButtons />
			{props.children}
		</>
	);
};

const OperatingSystemButtons = () => {
	return (
		<div class="field has-addons is-fullwidth">
			<p class="control">
				<OperatingSystemButton
					name="Linux"
					icon="fab fa-linux"
					operating_system={OperatingSystem.Linux}
				/>
			</p>
			<p class="control">
				<OperatingSystemButton
					name="MacOS"
					icon="fab fa-apple"
					operating_system={OperatingSystem.MacOS}
				/>
			</p>
			<p class="control">
				<OperatingSystemButton
					name="Windows"
					icon="fab fa-windows"
					operating_system={OperatingSystem.Windows}
				/>
			</p>
			<p class="control">
				<OperatingSystemButton
					name="Other"
					icon="fas fa-server"
					operating_system={OperatingSystem.Other}
				/>
			</p>
		</div>
	);
};

const OperatingSystemButton = (props: {
	name: string;
	icon: string;
	operating_system: OperatingSystem;
}) => {
	return (
		<button type="button" class="button">
			<span class="icon is-small">
				<i class={props.icon} />
			</span>
			<span>{props.name}</span>
		</button>
	);
};

export default InstallCliFallback;
