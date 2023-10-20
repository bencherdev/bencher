import { Show } from "solid-js";
import { isBencherCloud } from "../../util/ext";
import Redirect from "./Redirect";

const SelfHostedRedirect = (props: { path: string }) => (
	<Show when={!isBencherCloud()} fallback={<></>}>
		<Redirect path={props.path} />
	</Show>
);

export default SelfHostedRedirect;
