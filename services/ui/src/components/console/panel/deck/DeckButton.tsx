import { useNavigate } from "solid-app-router";
import { Match, Switch } from "solid-js";
import { ActionButton } from "../../config/types";
import { JsonAlertStatus } from "../../../../types/bencher";

const DeckButton = (props) => {
	const navigate = useNavigate();
	console.log(props.data());

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
	return (
		<Switch fallback={<></>}>
			<Match when={props.data()?.status === JsonAlertStatus.Unread}>
				<button
					class="button is-fullwidth is-primary"
					onClick={(e) => {
						e.preventDefault();
						// navigate(props.config.path(props.path_params));
					}}
				>
					Mark as Read
				</button>
			</Match>
			<Match when={props.data()?.status === JsonAlertStatus.Read}>
				<button
					class="button is-fullwidth is-outlined"
					onClick={(e) => {
						e.preventDefault();
						// navigate(props.config.path(props.path_params));
					}}
				>
					Mark as Unread
				</button>
			</Match>
		</Switch>
	);
};
export default DeckButton;
