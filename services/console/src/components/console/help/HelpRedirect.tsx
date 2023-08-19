import { authUser } from "../../../util/auth";
import AuthRedirect from "../../auth/AuthRedirect";

const HelpRedirect = () => (
	<AuthRedirect path={`/console/users/${authUser()?.user?.slug}/help`} />
);

export default HelpRedirect;
