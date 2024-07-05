import { removeUser } from "../../util/auth";
import { NotifyKind, navigateNotify } from "../../util/notify";
import { removeOrganization } from "../../util/organization";

const Logout = () => {
	removeUser();
	removeOrganization();
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
