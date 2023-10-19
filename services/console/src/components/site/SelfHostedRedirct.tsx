import { Show } from "solid-js";
import { isBencherCloud } from "../../util/ext";
import Redirect from "./Redirect";

const SelfHostedRedirect = (props: { apiUrl: string; path: string }) => (
	<Show when={!isBencherCloud(props.apiUrl)} fallback={<></>}>
		<Redirect path={props.path} />
	</Show>
);

export default SelfHostedRedirect;
