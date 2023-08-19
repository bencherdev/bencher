import { Switch, type Accessor, Match, Resource, createSignal } from "solid-js";
import { JsonAlertStatus, type JsonAuthUser } from "../../../../types/bencher";
import { validJwt } from "../../../../util/valid";
import { httpPatch } from "../../../../util/http";
import { useSearchParams } from "../../../../util/url";
import {
	NOTIFY_KIND_PARAM,
	NOTIFY_TEXT_PARAM,
	NotifyKind,
} from "../../../../util/notify";

export interface Props {
	user: JsonAuthUser;
	url: Accessor<string>;
	data: Resource<Record<string, any>>;
	handleRefresh: () => void;
}

const StatusButton = (props: Props) => {
	const [_searchParams, setSearchParams] = useSearchParams();

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
		const isActive = props.data()?.status === JsonAlertStatus.Active;
		httpPatch(props.url(), token, data)
			.then((_resp) => {
				setSubmitting(false);
				props.handleRefresh();
				setSearchParams({
					[NOTIFY_KIND_PARAM]: NotifyKind.OK,
					[NOTIFY_TEXT_PARAM]: isActive
						? "Phew, that was a hare-raising experience! Alert has been dismissed."
						: "We're not out of the woods yet! Alert has been reactivated.",
				});
			})
			.catch((error) => {
				setSubmitting(false);
				console.error(error);
				setSearchParams({
					[NOTIFY_KIND_PARAM]: NotifyKind.ERROR,
					[NOTIFY_TEXT_PARAM]: `Lettuce romaine calm! Failed to ${
						isActive ? "dismiss" : "reactivate"
					} alert. Please, try again.`,
				});
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
