import { useLocation, useNavigate } from "solid-app-router";
import { Match, Switch, createMemo, createSignal } from "solid-js";
import { JsonAlertStatus } from "../../../../types/bencher";
import axios from "axios";
import { NotifyKind, patch_options, validate_jwt } from "../../../site/util";
import { notification_path } from "../../../site/Notification";

const StatusButton = (props) => {
	const navigate = useNavigate();
	const location = useLocation();
	const pathname = createMemo(() => location.pathname);

	const [submitting, setSubmitting] = createSignal(false);

	const patch = async (data) => {
		const token = props.user?.token;
		if (!validate_jwt(token)) {
			return;
		}
		return await axios(patch_options(props.url(), token, data));
	};

	function send_status(e) {
		e.preventDefault();

		setSubmitting(true);
		let data;
		switch (props.data()?.status) {
			case JsonAlertStatus.Active:
				data = { status: JsonAlertStatus.Dismissed };
				break;
			case JsonAlertStatus.Dismissed:
				data = { status: JsonAlertStatus.Active };
				break;
			default:
				console.error("Unknown status");
		}

		patch(data)
			.then((_resp) => {
				setSubmitting(false);
				const status = props.data()?.status;
				props.handleRefresh();
				navigate(
					notification_path(
						pathname(),
						[],
						[],
						NotifyKind.OK,
						`Alert has been ${
							status === JsonAlertStatus.Active ? "dismissed" : "reactivated"
						}!`,
					),
				);
			})
			.catch((error) => {
				setSubmitting(false);
				console.error(error);
				const status = props.data()?.status;
				navigate(
					notification_path(
						pathname(),
						[],
						[],
						NotifyKind.ERROR,
						`Failed to ${
							status === JsonAlertStatus.Active ? "dismiss" : "reactivate"
						} alert. Please, try again.`,
					),
				);
			});
	}

	return (
		<Switch fallback={<></>}>
			<Match when={props.data()?.status === JsonAlertStatus.Active}>
				<button
					class="button is-primary is-fullwidth"
					title="Dismiss alert"
					disabled={submitting()}
					onClick={send_status}
				>
					<span class="icon">
						<i class="far fa-bell" aria-hidden="true" />
					</span>
					<span>Dismiss</span>
				</button>
			</Match>
			<Match when={props.data()?.status === JsonAlertStatus.Dismissed}>
				<button
					class="button is-outlined is-fullwidth"
					title="Reactivate alert"
					disabled={submitting()}
					onClick={send_status}
				>
					<span class="icon">
						<i class="far fa-bell-slash" aria-hidden="true" />
					</span>
					<span>Reactivate</span>
				</button>
			</Match>
		</Switch>
	);
};
export default StatusButton;
