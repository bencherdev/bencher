import { hiddenRedirect } from "../../util/url";

const Logout = () => {
    window.localStorage.clear();
    hiddenRedirect("/auth/login");

    return <></>
};

export default Logout;