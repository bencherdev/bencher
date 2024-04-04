import { createMemo } from "solid-js";
import { authUser } from "../../../util/auth";
import Redirect from "../../site/Redirect";

const ShortcutRedirect = (props: { path?: string }) => {
	const user = authUser();

	const path = createMemo(() =>
		authUser()?.token
			? `/console/users/${user?.user?.slug}/${props.path ?? ""}`
			: "/auth/login",
	);

	return <Redirect path={path()} />;
};

export default ShortcutRedirect;
