import Button from "@bencherdev/ui/Button";
import ButtonLink from "@bencherdev/ui/ButtonLink";
import Heading from "@bencherdev/ui/Heading";
import { Match, Show, Switch, createSignal } from "solid-js";
import { render } from "solid-js/web";
import { takeAuthReturn } from "../astro/authReturn";
import { handOff } from "../handoff";
import { deleteCache } from "../idb";
import { ask } from "./request";
import Frame from "./Frame";

const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;
const JWT = /^[\w-]+\.[\w-]+\.[\w-]+$/;

type Confirmed = "confirming" | "expired" | "used" | "taken" | "failed";

/** A new email's confirm link: nothing changes before the press, and the door follows it. */
const ConfirmEmail = (props: { user: string; token: string }) => {
	const [state, setState] = createSignal<Confirmed>();
	const valid = UUID.test(props.user) && JWT.test(props.token);
	const confirm = () => {
		setState("confirming");
		ask("PUT", `/v0/users/${props.user}/email`, { token: props.token }).then(
			(answer) => {
				if (answer.ok) {
					// The console's saved responses still hold the email just replaced.
					deleteCache();
					location.replace("/auth/login");
					return;
				}
				switch (answer.refused.status) {
					case 401:
						setState("expired");
						return;
					case 403:
						setState("used");
						return;
					case 409:
						setState("taken");
						return;
					default:
						setState("failed");
				}
			},
		);
	};
	return (
		<Show
			when={valid}
			fallback={
				<Heading level={1} size="xl">
					This link is not valid
				</Heading>
			}
		>
			<Show when={state() === undefined}>
				<Heading level={1} size="xl">
					Confirm your new email
				</Heading>
				<div class="au-row">
					<ButtonLink size="lg" href="/auth/login">
						Cancel
					</ButtonLink>
					<Button variant="primary" size="lg" onClick={confirm}>
						Change email
					</Button>
				</div>
			</Show>
			<div role="status">
				<Switch>
					<Match when={state() === "confirming"}>
						<Heading level={1} size="xl">
							Confirming your new email
						</Heading>
					</Match>
					<Match when={state() === "expired"}>
						<Heading level={1} size="xl">
							This link has expired
						</Heading>
					</Match>
					<Match when={state() === "used"}>
						<Heading level={1} size="xl">
							This link is no longer valid
						</Heading>
					</Match>
					<Match when={state() === "taken"}>
						<Heading level={1} size="xl">
							That email is already in use
						</Heading>
					</Match>
					<Match when={state() === "failed"}>
						<Heading level={1} size="xl">
							Failed to confirm your new email
						</Heading>
					</Match>
				</Switch>
			</div>
		</Show>
	);
};

const mount = document.getElementById("auth");
if (mount) {
	const { user, token } = takeAuthReturn();
	handOff(mount, () =>
		render(
			() => (
				<Frame>
					<ConfirmEmail user={user ?? ""} token={token ?? ""} />
				</Frame>
			),
			mount,
		),
	);
}
