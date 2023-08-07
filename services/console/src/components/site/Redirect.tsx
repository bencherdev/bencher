import { hiddenRedirect } from "../../util/url";

const Redirect = (props: { path: string }) => {
	hiddenRedirect(props.path);

	return <></>;
};

export default Redirect;
