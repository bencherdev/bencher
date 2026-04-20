import * as Sentry from "@sentry/astro";
import { type Accessor, type Resource, Show, createSignal } from "solid-js";
import type { JsonAuthUser, JsonToken } from "../../../../types/bencher";
import { httpDelete } from "../../../../util/http";
import { NotifyKind, pageNotify } from "../../../../util/notify";
import { validJwt } from "../../../../util/valid";

export interface Props {
	apiUrl: string;
	user: JsonAuthUser;
	path: Accessor<string>;
	data: Resource<JsonToken>;
	subtitle: string;
	isAllowed: Resource<boolean>;
	handleRefresh: () => void;
}

const RevokeButton = (props: Props) => {
	const [revokeClicked, setRevokeClicked] = createSignal(false);
	const [revoking, setRevoking] = createSignal(false);

	const sendRevoke = () => {
		setRevoking(true);
		const data = props.data();
		if (!data) {
			setRevoking(false);
			return;
		}

		const token = props.user?.token;
		if (!validJwt(token)) {
			setRevoking(false);
			return;
		}

		httpDelete(props.apiUrl, props.path(), token)
			.then((_resp) => {
				setRevoking(false);
				setRevokeClicked(false);
				props.handleRefresh();
			})
			.catch((error) => {
				setRevoking(false);
				console.error(error);
				Sentry.captureException(error);
				pageNotify(
					NotifyKind.ERROR,
					`Lettuce romaine calm! Failed to revoke: ${error?.response?.data?.message}`,
				);
			});
	};

	return (
		<Show when={!props.data()?.revoked}>
			<Show when={props.isAllowed()}>
				<Show
					when={revokeClicked()}
					fallback={
						<div class="buttons is-right">
							<button
								class="button is-small"
								type="button"
								onMouseDown={(e) => {
									e.preventDefault();
									setRevokeClicked(true);
								}}
							>
								<span class="icon">
									<i class="fas fa-ban" />
								</span>
								<span>Revoke</span>
							</button>
						</div>
					}
				>
					<div class="content has-text-centered">
						<h3 class="title is-3">Are you sure? Revocation is permanent.</h3>
						{props.subtitle && <h4 class="subtitle is-4">{props.subtitle}</h4>}
					</div>
					<div class="columns">
						<div class="column">
							<button
								class="button is-fullwidth"
								type="submit"
								disabled={revoking()}
								onMouseDown={(e) => {
									e.preventDefault();
									sendRevoke();
								}}
							>
								I am 💯 sure
							</button>
						</div>
						<div class="column">
							<button
								class="button is-primary is-fullwidth"
								type="button"
								onMouseDown={(e) => {
									e.preventDefault();
									setRevokeClicked(false);
								}}
							>
								Cancel
							</button>
						</div>
					</div>
				</Show>
			</Show>
		</Show>
	);
};

export default RevokeButton;
