import { render } from "solid-js/web";
import {
	type ConsentRequest,
	consentErrorText,
	consentRequest,
} from "../../components/auth/authorize/consent";
import { keepOutOfReports } from "../../util/scrub";
import { handOff } from "../handoff";
import { deleteCache } from "../idb";
import { forgetReader } from "../memory";
import { readReader } from "../reader";
import { endSession } from "../session";
import Consent, { Ended, consentEffects } from "./Consent";
import Frame from "./Frame";

const boot = (mount: HTMLElement) => {
	const request: ConsentRequest = consentRequest(
		new URLSearchParams(location.search),
	);
	// The reports this page sends leave out the request's UUID, which is all it
	// takes to answer it. The reporter loads later, with the page idle.
	keepOutOfReports("uuid" in request ? [request.uuid] : []);
	// Only a hint names a reader to forget; a visitor with none keeps what this browser holds.
	const forget = () => {
		if (readReader(localStorage)) {
			forgetReader(localStorage);
			deleteCache();
		}
	};
	const effects = consentEffects(async () => {
		await endSession();
		forget();
	}, forget);
	handOff(mount, () =>
		render(
			() => (
				<Frame>
					{"uuid" in request ? (
						<Consent uuid={request.uuid} effects={effects} />
					) : (
						<Ended text={consentErrorText(request.error)} />
					)}
				</Frame>
			),
			mount,
		),
	);
};

const mount = document.getElementById("auth");
if (mount) {
	boot(mount);
}
