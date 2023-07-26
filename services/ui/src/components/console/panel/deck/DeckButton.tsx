import { useLocation, useNavigate } from "solid-app-router";
import { Match, Switch, createMemo, createSignal } from "solid-js";
import { ActionButton } from "../../config/types";
import { JsonAlertStatus } from "../../../../types/bencher";
import axios from "axios";
import {
	BENCHER_API_URL,
	NotifyKind,
	patch_options,
	validate_jwt,
} from "../../../site/util";
import { notification_path } from "../../../site/Notification";

const DeckButton = (props) => {
	return (
		<div class="columns">
			<div class="column">
				<div class="box">
					<div class="columns">
						<div class="column">
							<Switch fallback={<></>}>
								<Match when={props.config?.kind === ActionButton.ToggleRead}>
									<ToggleReadButton {...props} />
								</Match>
							</Switch>
						</div>
					</div>
				</div>
			</div>
		</div>
	);
};

const ToggleReadButton = (props) => {
	const navigate = useNavigate();
	const location = useLocation();
	const pathname = createMemo(() => location.pathname);

	const url = createMemo(
		() =>
			`${BENCHER_API_URL()}/v0/projects/${
				props.path_params?.project_slug
			}/alerts/${props.path_params?.alert_uuid}`,
	);

	const [submitting, setSubmitting] = createSignal(false);

	const patch = async (data) => {
		const token = props.user?.token;
		if (!validate_jwt(token)) {
			return;
		}
		return await axios(patch_options(url(), token, data));
	};

	function sendForm(e) {
		e.preventDefault();

		setSubmitting(true);
		let data;
		switch (props.data()?.status) {
			case JsonAlertStatus.Unread:
				data = { status: JsonAlertStatus.Read };
				break;
			case JsonAlertStatus.Read:
				data = { status: JsonAlertStatus.Unread };
				break;
			default:
				console.error("Unknown status");
		}
		console.log(data);

		patch(data)
			.then((_resp) => {
				setSubmitting(false);
				props.handleRefresh();
				navigate(
					notification_path(
						pathname(),
						[],
						[],
						NotifyKind.OK,
						"Update successful!",
					),
				);
			})
			.catch((error) => {
				setSubmitting(false);
				console.error(error);
				navigate(
					notification_path(
						pathname(),
						[],
						[],
						NotifyKind.ERROR,
						"Failed to update. Please, try again.",
					),
				);
			});
	}

	return (
		<Switch fallback={<></>}>
			<Match when={props.data()?.status === JsonAlertStatus.Unread}>
				<button
					class="button is-fullwidth is-primary"
					title="Mark alert as read"
					disabled={submitting()}
					onClick={sendForm}
				>
					Mark as Read
				</button>
			</Match>
			<Match when={props.data()?.status === JsonAlertStatus.Read}>
				<button
					class="button is-fullwidth is-outlined"
					title="Mark alert as unread"
					disabled={submitting()}
					onClick={sendForm}
				>
					Mark as Unread
				</button>
			</Match>
		</Switch>
	);
};
export default DeckButton;
