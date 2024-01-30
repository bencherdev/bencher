import { Show, createMemo } from "solid-js";
import {
	NOTIFY_KIND_PARAM,
	NOTIFY_TEXT_PARAM,
	NotifyKind,
	isNotifyKind,
	isNotifyText,
} from "../../util/notify";
import { pathname, useSearchParams } from "../../util/url";

const Notification = (props: { suppress?: undefined | boolean }) => {
	const [searchParams, setSearchParams] = useSearchParams();

	const initParams: Record<string, null> = {};
	if (!isNotifyKind(searchParams[NOTIFY_KIND_PARAM])) {
		initParams[NOTIFY_KIND_PARAM] = null;
	}
	if (!isNotifyText(searchParams[NOTIFY_TEXT_PARAM])) {
		initParams[NOTIFY_TEXT_PARAM] = null;
	}
	if (Object.keys(initParams).length !== 0) {
		setSearchParams(initParams, { replace: true });
	}

	const notifyKind = createMemo(() => searchParams[NOTIFY_KIND_PARAM]);
	const notifyText = createMemo(() => searchParams[NOTIFY_TEXT_PARAM]);
	const suppress = createMemo(() =>
		typeof props.suppress === "boolean" ? props.suppress : false,
	);

	const removeNotification = () => {
		// Check to see if the pathname is still the same
		// Otherwise, this whiplashes the user back to the page that generated the notification
		if (pathname() === window.location.pathname) {
			setSearchParams(
				{
					[NOTIFY_KIND_PARAM]: null,
					[NOTIFY_TEXT_PARAM]: null,
				},
				{ replace: true },
			);
		}
	};

	const notification = () => {
		let color: string;
		switch (notifyKind()) {
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
		}, 4321);
		return (
			<div class={`notification ${color}`}>
				🐰 {notifyText()}
				<button
					class="delete"
					type="button"
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
			<Show
				when={
					!suppress() &&
					isNotifyKind(notifyKind()) &&
					isNotifyText(notifyText())
				}
			>
				<section class="section">
					<div class="container">{notification()}</div>
				</section>
			</Show>
		</div>
	);
};

export default Notification;
