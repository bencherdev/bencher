import { Show } from "solid-js";
import { authUser } from "../../util/auth";
import { decodePath } from "../../util/url";

const AuthButtons = () => {
	return (
		<Show
			when={authUser()?.token}
			fallback={
				<div class="buttons">
					<a class="button" href="/auth/login">
						Log in
					</a>
					<a class="button is-primary" href="/auth/signup">
						<strong>Sign up</strong>
					</a>
				</div>
			}
		>
			<a class="button" href={decodePath("/console")}>
				<span class="icon has-text-primary">
					<i class="fas fa-angle-left" />
				</span>
				<span>Back to Console</span>
			</a>
		</Show>
	);
};

export default AuthButtons;
