import { Show } from "solid-js";
import { authUser } from "../../util/auth";

const PricingLink = () => {
	return (
		<Show
			when={authUser()?.token}
			fallback={
				<a class="navbar-item" href="/pricing/">
					Pricing
				</a>
			}
		>
			<div></div>
		</Show>
	);
};

export default PricingLink;
