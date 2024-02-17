import {
	NOTIFY_KIND_PARAM,
	NOTIFY_TEXT_PARAM,
	forwardParams,
} from "../../util/notify";
import { BACK_PARAM, hiddenRedirect } from "../../util/url";
import { PLAN_PARAM } from "../auth/auth";

const Redirect = (props: { path: string }) => {
	hiddenRedirect(
		forwardParams(
			props.path,
			[NOTIFY_KIND_PARAM, NOTIFY_TEXT_PARAM, PLAN_PARAM, BACK_PARAM],
			null,
		),
	);

	return <></>;
};

export default Redirect;
