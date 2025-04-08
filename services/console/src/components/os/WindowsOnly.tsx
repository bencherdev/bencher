import { createResource, Show } from "solid-js";
import { getOperatingSystem, OperatingSystem } from "./operating_system";

const WindowsOnly = (props: { children }) => {
	const [os] = createResource(getOperatingSystem);

	return <Show when={os() === OperatingSystem.Windows}>{props.children}</Show>;
};

export default WindowsOnly;
