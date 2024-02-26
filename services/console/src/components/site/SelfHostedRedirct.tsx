import { Show } from "solid-js";
import { isBencherCloud } from "../../util/ext";
import Redirect from "./Redirect";
import { ApiCollections, Collection } from "../../content/config";

const SelfHostedRedirect = (props: {
	path: string;
	collection: undefined | Collection;
}) => (
	<Show
		when={
			!isBencherCloud() &&
			(props.collection ? !ApiCollections.includes(props.collection) : true)
		}
	>
		<Redirect path={props.path} />
	</Show>
);

export default SelfHostedRedirect;
