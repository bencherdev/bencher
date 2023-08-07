import { authUser } from "../../util/auth";
import Redirect from "../site/Redirect";

const UsersRoot = () => {
	const path = `/console/users/${authUser()?.user?.slug}/settings`;
	return <Redirect path={path} />;
};

export default UsersRoot;
