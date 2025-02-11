import * as Sentry from "@sentry/astro";
import { type Accessor, type Resource, Show, createSignal } from "solid-js";
import type { JsonAuthUser, JsonThreshold } from "../../../../types/bencher";
import { httpPut } from "../../../../util/http";
import { NotifyKind, pageNotify } from "../../../../util/notify";
import { validJwt } from "../../../../util/valid";

export interface Props {
	apiUrl: string;
	user: JsonAuthUser;
	path: Accessor<string>;
	data: Resource<JsonThreshold>;
	subtitle: string;
	redirect: (pathname: string, data: object) => string;
	notify?: boolean;
	effect?: undefined | (() => void);
	isAllowed: Resource<boolean>;
	handleRefresh: () => void;
}

const RemoveModelButton = (props: Props) => {
	const [removing, setRemoving] = createSignal(false);

	const sendRemove = () => {
		setRemoving(true);
		const data = props.data();
		// This guarantees that the wasm has been loaded
		if (!data) {
			setRemoving(false);
			return;
		}

		const token = props.user?.token;
		if (!validJwt(token)) {
			setRemoving(false);
			return;
		}

		httpPut(props.apiUrl, props.path(), token, { test: null })
			.then((_resp) => {
				setRemoving(false);
				props.handleRefresh();
			})
			.catch((error) => {
				setRemoving(false);
				console.error(error);
				Sentry.captureException(error);
				pageNotify(
					NotifyKind.ERROR,
					`Lettuce romaine calm! Failed to remove model: ${error?.response?.data?.message}`,
				);
			});
	};

	return (
		<Show when={props.data()?.model && !props?.data()?.model?.replaced}>
			<div class="buttons is-right">
				<button
					class="button is-small"
					type="button"
					disabled={!props.isAllowed() || removing()}
					onMouseDown={(e) => {
						e.preventDefault();
						sendRemove();
					}}
				>
					<span class="fa-stack fa-2x" style="font-size: 0.75em;">
						<i class="fas fa-walking fa-stack-1x" />
						<i class="fas fa-ban fa-stack-2x" />
					</span>
					<span>&nbsp;Reset Threshold</span>
				</button>
			</div>
		</Show>
	);
};

export default RemoveModelButton;
