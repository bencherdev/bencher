import { removeUser } from "../../util/auth";
import { NotifyKind, navigateNotify } from "../../util/notify";

const Logout = () => {
	removeUser();
	navigateNotify(
		NotifyKind.OK,
		"Lettuce meet again!",
		"/auth/login",
		null,
		null,
		true,
	);

	return <></>;
};

export default Logout;
