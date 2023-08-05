import { removeUser } from "../../util/auth";
import { hiddenRedirect } from "../../util/url";

const Logout = () => {
    removeUser();
    hiddenRedirect("/auth/login");

    return <></>
};

export default Logout;