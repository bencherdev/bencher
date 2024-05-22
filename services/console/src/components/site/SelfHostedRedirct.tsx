import { Show } from "solid-js";
import { isBencherCloud } from "../../util/ext";
import Redirect from "./Redirect";
import type Collection from "../../util/collection";
import { ApiCollections } from "../../util/collection";

const SelfHostedRedirect = (props: {
	path: string;
	collection?: undefined | Collection;
}) => {
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
