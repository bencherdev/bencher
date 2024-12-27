import { Show, createResource } from "solid-js";
import { type Platform, getPlatform } from "../../util/platform";

const ClientPlatform = (props: { platform: Platform; children }) => {
	const [platform] = createResource(getPlatform);

	return <Show when={props.platform === platform()}>{props.children}</Show>;
};

export default ClientPlatform;
