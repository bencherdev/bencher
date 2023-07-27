import { useLocation, useNavigate } from "solid-app-router";
import { Match, Switch, createMemo, createSignal } from "solid-js";
import { ActionButton } from "../../config/types";
import { JsonAlertStatus } from "../../../../types/bencher";
import axios from "axios";
import {
	BENCHER_API_URL,
	NotifyKind,
	delete_options,
	patch_options,
	validate_jwt,
} from "../../../site/util";
import { notification_path } from "../../../site/Notification";

const DeckButton = (props) => {
	return (
		<div class="columns">
			<div class="column">
				<form class="box">
					<div class="field">
						<p class="control">
							<Switch fallback={<></>}>
								<Match when={props.config?.kind === ActionButton.DELETE}>
									<DeleteButton
										subtitle={props.config.subtitle}
										user={props.user}
										url={props.url}
										path={props.config.path}
										data={props.data}
									/>
								</Match>
							</Switch>
						</p>
					</div>
				</form>
			</div>
		</div>
	);
};

const DeleteButton = (props) => {
	const navigate = useNavigate();
	const location = useLocation();
	const pathname = createMemo(() => location.pathname);

	const [delete_clicked, setDeleteClicked] = createSignal(false);
	const [deleting, setDeleting] = createSignal(false);

	const deletion = async () => {
		const token = props.user?.token;
		if (!validate_jwt(token)) {
			return;
		}
		const url = props.url();
		return await axios(delete_options(url, token));
	};

	function send_delete(e) {
		e.preventDefault();

		setDeleting(true);

		deletion()
			.then((_resp) => {
				setDeleting(false);
				navigate(
					notification_path(
						props.path(pathname(), props.data()),
						[],
						[],
						NotifyKind.OK,
						"Delete successful!",
					),
				);
			})
			.catch((error) => {
				setDeleting(false);
				console.error(error);
				navigate(
					notification_path(
						pathname(),
						[],
						[],
						NotifyKind.ERROR,
						"Failed to delete. Please, try again.",
					),
				);
			});
	}

	return (
		<Switch fallback={<></>}>
			<Match when={delete_clicked() === false}>
				<button
					class="button is-danger is-fullwidth"
					onClick={(e) => {
						e.preventDefault();
						setDeleteClicked(true);
					}}
				>
					Delete
				</button>
			</Match>
			<Match when={delete_clicked() === true}>
				<div class="content has-text-centered">
					<h3 class="title">Are you sure? This is permanent.</h3>
					{props.subtitle && <h4 class="subtitle">{props.subtitle}</h4>}
				</div>
				<div class="columns">
					<div class="column">
						<button
							class="button is-fullwidth"
							disabled={deleting()}
							onClick={send_delete}
						>
							I am 💯 sure
						</button>
					</div>
					<div class="column">
						<button
							class="button is-primary is-fullwidth"
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

export default DeckButton;
