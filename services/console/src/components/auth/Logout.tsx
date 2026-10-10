import { deleteCache } from "../../next/idb";
import { forgetShell } from "../../next/memory";
import { removeUser } from "../../util/auth";
import { NotifyKind, navigateNotify } from "../../util/notify";
import { removeOrganization } from "../../util/organization";

const Logout = () => {
	removeUser();
	removeOrganization();
	// What the new console remembers of this reader goes with them.
	forgetShell(localStorage);
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
