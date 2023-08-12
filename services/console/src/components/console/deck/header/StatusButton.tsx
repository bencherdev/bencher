import { Switch, type Accessor, Match, Resource, createSignal } from "solid-js";
import { JsonAlertStatus, type JsonAuthUser } from "../../../../types/bencher";
import { validJwt } from "../../../../util/valid";
import { httpPatch } from "../../../../util/http";
import { pathname, useNavigate } from "../../../../util/url";

export interface Props {
	user: JsonAuthUser;
	url: Accessor<string>;
	data: Resource<Record<string, any>>;
	handleRefresh: () => void;
}

const StatusButton = (props: Props) => {
	const navigate = useNavigate();
	// const location = useLocation();
	// const pathname = createMemo(() => location.pathname);

	const [submitting, setSubmitting] = createSignal(false);

	const getStatus = () => {
		switch (props.data()?.status) {
			case JsonAlertStatus.Active:
				return { status: JsonAlertStatus.Dismissed };
			case JsonAlertStatus.Dismissed:
				return { status: JsonAlertStatus.Active };
			default:
				console.error("Unknown status");
				return;
		}
	};

	const sendStatus = () => {
		// Check the status first, the guarantees that the wasm has been initialized
		const data = getStatus();
		if (!data) {
			return;
		}
		const token = props.user?.token;
		if (!validJwt(token)) {
			return;
		}

		setSubmitting(true);
		httpPatch(props.url(), token, data)
			.then((_resp) => {
				setSubmitting(false);
				const status = props.data()?.status;
				// props.handleRefresh();
				navigate(pathname());
				// navigate(
				// 	notification_path(
				// 		pathname(),
				// 		[],
				// 		[],
				// 		NotifyKind.OK,
				// 		`Alert has been ${status === JsonAlertStatus.Active ? "dismissed" : "reactivated"
				// 		}!`,
				// 	),
				// );
			})
			.catch((error) => {
				setSubmitting(false);
				console.error(error);
				const status = props.data()?.status;
				// navigate(
				// 	notification_path(
				// 		pathname(),
				// 		[],
				// 		[],
				// 		NotifyKind.ERROR,
				// 		`Failed to ${status === JsonAlertStatus.Active ? "dismiss" : "reactivate"
				// 		} alert. Please, try again.`,
				// 	),
				// );
			});
	};

	return (
		<Switch fallback={<></>}>
			<Match when={props.data()?.status === JsonAlertStatus.Active}>
				<button
					class="button is-primary is-fullwidth"
					title="Dismiss alert"
					disabled={submitting()}
					onClick={(e) => {
						e.preventDefault();
						sendStatus();
					}}
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
					onClick={(e) => {
						e.preventDefault();
						sendStatus();
					}}
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
