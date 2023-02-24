import { useLocation, useSearchParams } from "solid-app-router";
import { createMemo, Match, Switch } from "solid-js";
import { forward_path } from "./Forward";
import {
	NotifyKind,
	NOTIFY_KIND_PARAM,
	NOTIFY_TEXT_PARAM,
	isNotifyKind,
	isNotifyText,
} from "./util";

export const notification_path = (
	href: string,
	keep_params: string[],
	set_params: string[][],
	notify_kind: NotifyKind,
	notify_text: string,
) => {
	set_params = [
		[NOTIFY_KIND_PARAM, notify_kind.toString()],
		[NOTIFY_TEXT_PARAM, notify_text],
		...set_params,
	];
	return forward_path(href, keep_params, set_params);
};

const Notification = (_props) => {
	const location = useLocation();
	const pathname = createMemo(() => location.pathname);
	const [searchParams, setSearchParams] = useSearchParams();

	if (!isNotifyKind(searchParams[NOTIFY_KIND_PARAM])) {
		setSearchParams({ [NOTIFY_KIND_PARAM]: null });
	}

	if (!isNotifyText(searchParams[NOTIFY_TEXT_PARAM])) {
		setSearchParams({ [NOTIFY_TEXT_PARAM]: null });
	}

	const notify_kind = createMemo(() =>
		parseInt(searchParams[NOTIFY_KIND_PARAM]),
	);

	const notify_text = createMemo(() => searchParams[NOTIFY_TEXT_PARAM]);

	const removeNotification = () => {
		// Check to see if the pathname is still the same
		// Otherwise, this whiplashes the user back to the page that generated the notification
		if (pathname() === window.location.pathname) {
			setSearchParams({
				[NOTIFY_KIND_PARAM]: null,
				[NOTIFY_TEXT_PARAM]: null,
			});
		}
	};

	const getNotification = () => {
		let color: string;
		switch (notify_kind()) {
			case NotifyKind.OK:
				color = "is-success";
				break;
			case NotifyKind.ALERT:
				color = "is-primary";
				break;
			case NotifyKind.ERROR:
				color = "is-danger";
				break;
			default:
				color = "";
		}
		setTimeout(() => {
			removeNotification();
		}, 4000);
		return (
			<div class={`notification ${color}`}>
				🐰 {notify_text()}
				<button
					class="delete"
					onClick={(e) => {
						e.preventDefault();
						removeNotification();
					}}
				/>
			</div>
		);
	};

	return (
		<div>
			<Switch fallback={<></>}>
				<Match
					when={isNotifyKind(notify_kind()) && isNotifyText(notify_text())}
				>
					<section class="section">
						<div class="container">{getNotification()}</div>
					</section>
				</Match>
			</Switch>
		</div>
	);
};

export default Notification;
