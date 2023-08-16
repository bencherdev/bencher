import { forwardParams } from "../../util/notify";
import { hiddenRedirect } from "../../util/url";
import { PLAN_PARAM } from "../auth/auth";

const Redirect = (props: { path: string }) => {
	hiddenRedirect(forwardParams(props.path, [PLAN_PARAM], null));

	return <></>;
};

export default Redirect;
