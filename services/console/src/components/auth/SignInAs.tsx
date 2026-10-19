import type { JsonAuthUser } from "../../types/bencher";

interface Props {
	user: JsonAuthUser;
	/** Where to go instead, signed in as no one. */
	cancel: string;
	onSignIn: () => void;
}

/** Asks before completing any sign in but the one this tab started. */
const SignInAs = (props: Props) => (
	<div class="box">
		<button
			class="button is-primary is-fullwidth"
			type="button"
			onClick={() => props.onSignIn()}
		>
			Sign in as {props.user.user.email}
		</button>
		<a
			class="button is-fullwidth"
			style="margin-top: 1rem;"
			href={props.cancel}
		>
			Cancel
		</a>
	</div>
);

export default SignInAs;
