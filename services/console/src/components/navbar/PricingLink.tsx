import { Show } from "solid-js";
import { authUser } from "../../util/auth";

const PricingLink = () => {
	return (
		<Show
			when={authUser()}
			fallback={
				<a class="navbar-item" href="/pricing/">
					Pricing
				</a>
			}
		>
			<div />
		</Show>
	);
};

export default PricingLink;
