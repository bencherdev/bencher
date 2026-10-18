import { deleteCache } from "../../next/idb";
import { forgetReader } from "../../next/memory";
import { NotifyKind, navigateNotify } from "../../util/notify";

const Logout = () => {
	forgetReader(localStorage);
	deleteCache();
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
