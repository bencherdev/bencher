import { authUser } from "../../../util/auth";
import UserMenuInner from "./UserMenuInner";

const UserMenu = () => {
	const user = authUser();

	return <UserMenuInner user={user} />;
};

export default UserMenu;
