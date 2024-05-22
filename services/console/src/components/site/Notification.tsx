import { Show, createEffect, createMemo } from "solid-js";
import {
	NOTIFY_KIND_PARAM,
	NOTIFY_LINK_TEXT_PARAM,
	NOTIFY_LINK_URL_PARAM,
	NOTIFY_TEXT_PARAM,
	NOTIFY_TIMEOUT,
	NOTIFY_TIMEOUT_PARAM,
	NotifyKind,
	isNotifyKind,
	isNotifyText,
	isNotifyTimeout,
} from "../../util/notify";
import { pathname, useSearchParams } from "../../util/url";

const Notification = (props: { suppress?: undefined | boolean }) => {
	const [searchParams, setSearchParams] = useSearchParams();

	createEffect(() => {
		const initParams: Record<string, null> = {};
		if (!isNotifyKind(searchParams[NOTIFY_KIND_PARAM])) {
			initParams[NOTIFY_KIND_PARAM] = null;
		}
		if (!isNotifyText(searchParams[NOTIFY_TEXT_PARAM])) {
			initParams[NOTIFY_TEXT_PARAM] = null;
		}
		if (!isNotifyTimeout(searchParams[NOTIFY_TIMEOUT_PARAM])) {
			initParams[NOTIFY_TIMEOUT_PARAM] = null;
		}
		if (!isNotifyText(searchParams[NOTIFY_LINK_URL_PARAM])) {
			initParams[NOTIFY_LINK_URL_PARAM] = null;
		}
		if (!isNotifyText(searchParams[NOTIFY_LINK_TEXT_PARAM])) {
			initParams[NOTIFY_LINK_TEXT_PARAM] = null;
		}
		if (Object.keys(initParams).length !== 0) {
			setSearchParams(initParams);
		}
	});

	const notifyKind = createMemo(() => searchParams[NOTIFY_KIND_PARAM]);
	const notifyText = createMemo(() => searchParams[NOTIFY_TEXT_PARAM]);
	const notifyTimeout = createMemo(() =>
		Number.parseInt(searchParams[NOTIFY_TIMEOUT_PARAM] ?? `${NOTIFY_TIMEOUT}`),
	);
	const notifyLinkUrl = createMemo(() => searchParams[NOTIFY_LINK_URL_PARAM]);
	const notifyLinkText = createMemo(() => searchParams[NOTIFY_LINK_TEXT_PARAM]);
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
					[NOTIFY_TIMEOUT_PARAM]: null,
					[NOTIFY_LINK_URL_PARAM]: null,
					[NOTIFY_LINK_TEXT_PARAM]: null,
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
		}, notifyTimeout());
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
				{notifyLinkUrl() && notifyLinkText() && (
					<>
						<div class="content has-text-centered" style="margin-top: 1rem;">
							<div class="columns is-centered">
								<div class="column is-half">
									<a
										class="button is-primary is-fullwidth"
										href={notifyLinkUrl()}
									>
										{notifyLinkText()}
									</a>
								</div>
							</div>
						</div>
					</>
				)}
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
