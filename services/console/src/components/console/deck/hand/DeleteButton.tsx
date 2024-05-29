import {
	type Accessor,
	Match,
	type Resource,
	Switch,
	createSignal,
} from "solid-js";
import type { JsonAuthUser } from "../../../../types/bencher";
import { httpDelete } from "../../../../util/http";
import {
	NotifyKind,
	navigateNotify,
	pageNotify,
} from "../../../../util/notify";
import { pathname } from "../../../../util/url";
import { validJwt } from "../../../../util/valid";

export interface Props {
	apiUrl: string;
	user: JsonAuthUser;
	path: Accessor<string>;
	data: Resource<object>;
	subtitle: string;
	redirect: (pathname: string, data: object) => string;
}

const DeleteButton = (props: Props) => {
	const [deleteClicked, setDeleteClicked] = createSignal(false);
	const [deleting, setDeleting] = createSignal(false);

	const sendDelete = () => {
		setDeleting(true);
		const data = props.data();
		// This guarantees that the wasm has been loaded
		if (!data) {
			return;
		}

		const token = props.user?.token;
		if (!validJwt(token)) {
			return;
		}

		httpDelete(props.apiUrl, props.path(), token)
			.then((_resp) => {
				setDeleting(false);
				navigateNotify(
					NotifyKind.OK,
					"That won't turnip again. Delete successful!",
					props.redirect(pathname(), data),
					null,
					null,
				);
			})
			.catch((error) => {
				setDeleting(false);
				console.error(error);
				pageNotify(
					NotifyKind.ERROR,
					"Lettuce romaine calm! Failed to delete. Please, try again.",
				);
			});
	};

	return (
		<Switch>
			<Match when={deleteClicked() === false}>
				<button
					class="button is-danger is-fullwidth is-small"
					type="button"
					onClick={(e) => {
						e.preventDefault();
						setDeleteClicked(true);
					}}
				>
					Delete
				</button>
			</Match>
			<Match when={deleteClicked() === true}>
				<div class="content has-text-centered">
					<h3 class="title is-3">Are you sure? This is permanent.</h3>
					{props.subtitle && <h4 class="subtitle is-4">{props.subtitle}</h4>}
				</div>
				<div class="columns">
					<div class="column">
						<button
							class="button is-fullwidth"
							type="submit"
							disabled={deleting()}
							onClick={(e) => {
								e.preventDefault();
								sendDelete();
							}}
						>
							I am 💯 sure
						</button>
					</div>
					<div class="column">
						<button
							class="button is-primary is-fullwidth"
							type="button"
							onClick={(e) => {
								e.preventDefault();
								setDeleteClicked(false);
							}}
						>
							Cancel
						</button>
					</div>
				</div>
			</Match>
		</Switch>
	);
};

export default DeleteButton;
