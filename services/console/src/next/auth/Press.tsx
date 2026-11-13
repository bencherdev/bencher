import Banner from "@bencherdev/ui/Banner";
import Button from "@bencherdev/ui/Button";
import ButtonLink from "@bencherdev/ui/ButtonLink";
import Heading from "@bencherdev/ui/Heading";
import { Show } from "solid-js";

/** Where Cancel and a way back go: the console, as the signed-in reader left it. */
export const CONSOLE = "/console";

/**
 * One question and one press, as Sign in as asks: what the press does, the
 * press, and Cancel back to the console.
 */
export const Press = (props: {
	heading: string;
	press: string;
	busy: boolean;
	failure: string | undefined;
	onPress: () => void;
}) => (
	<>
		<Heading level={1} size="xl">
			{props.heading}
		</Heading>
		<div class="au-stack">
			<Button
				variant="primary"
				size="lg"
				full
				disabled={props.busy}
				onClick={() => props.onPress()}
			>
				{props.press}
			</Button>
			<ButtonLink size="lg" full href={CONSOLE}>
				Cancel
			</ButtonLink>
			<Show when={props.failure}>
				{(failure) => (
					<Banner status="error" role="alert">
						{failure()}
					</Banner>
				)}
			</Show>
		</div>
	</>
);

/** A press that cannot be pressed, which says why, with a way back to the console. */
export const Refused = (props: { heading: string }) => (
	<>
		<Heading level={1} size="xl">
			{props.heading}
		</Heading>
		<div class="au-stack">
			<ButtonLink variant="primary" size="lg" full href={CONSOLE}>
				Back to the console
			</ButtonLink>
		</div>
	</>
);
