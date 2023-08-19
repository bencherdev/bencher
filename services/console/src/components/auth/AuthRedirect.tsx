import { Show } from "solid-js";
import Redirect from "../site/Redirect";
import { authUser } from "../../util/auth";

const AuthRedirect = (props: { path: string }) => (
	<Show when={authUser()?.token} fallback={<></>}>
		<Redirect path={props.path} />
	</Show>
);

export default AuthRedirect;
