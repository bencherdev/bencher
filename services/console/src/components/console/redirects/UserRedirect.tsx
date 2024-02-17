import { Show, createMemo } from "solid-js";
import { authUser } from "../../../util/auth";
import Redirect from "../../site/Redirect";

const UserRedirect = (props: { path: string }) => {
	const user = authUser();

	const path = createMemo(
		() => `/console/users/${user?.user?.slug}/${props.path}`,
	);

	return (
		<Show when={authUser()?.token}>
			<Redirect path={path()} />
		</Show>
	);
};

export default UserRedirect;
