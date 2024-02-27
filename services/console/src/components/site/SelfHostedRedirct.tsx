import { Show } from "solid-js";
import { isBencherCloud } from "../../util/ext";
import Redirect from "./Redirect";
import Collection, { ApiCollections } from "../../util/collection";

const SelfHostedRedirect = (props: {
	path: string;
	collection?: undefined | Collection;
}) => {
	console.log(props.collection);
	return (
		<Show
			when={
				!isBencherCloud() &&
				(props.collection ? !ApiCollections.includes(props.collection) : true)
			}
		>
			<Redirect path={props.path} />
		</Show>
	);
};

export default SelfHostedRedirect;
