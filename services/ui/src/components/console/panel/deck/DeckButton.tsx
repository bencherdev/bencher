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
								<Match when={props.config?.kind === ActionButton.DELETE}>
									TODO
								</Match>
							</Switch>
						</div>
					</div>
				</div>
			</div>
		</div>
	);
};

export default DeckButton;
