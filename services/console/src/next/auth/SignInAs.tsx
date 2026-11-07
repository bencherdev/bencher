import Button from "@bencherdev/ui/Button";
import ButtonLink from "@bencherdev/ui/ButtonLink";
import Heading from "@bencherdev/ui/Heading";

/** Asks before a link signs in a browser that did not ask for it. */
const SignInAs = (props: {
	email: string;
	busy?: boolean | undefined;
	onSignIn: () => void;
	/** Where to go instead, signed in as no one. */
	cancel: string;
}) => (
	<div class="au-stack">
		{/* The board draws no title, but a page needs one to be found by. */}
		<Heading level={1} size="xl" class="sr-only">
			Sign in to Bencher
		</Heading>
		<Button
			variant="primary"
			size="lg"
			full
			disabled={props.busy}
			onClick={() => props.onSignIn()}
		>
			Sign in as {props.email}
		</Button>
		<ButtonLink size="lg" full href={props.cancel}>
			Cancel
		</ButtonLink>
	</div>
);

export default SignInAs;
