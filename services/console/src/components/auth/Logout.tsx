import { NotifyKind, navigateNotify } from "../../util/notify";
import { signOut } from "./signOut";

const Logout = () => {
	signOut();
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
