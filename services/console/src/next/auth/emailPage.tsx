import { render } from "solid-js/web";
import { takeEmailChange } from "../astro/emailReturn";
import { handOff } from "../handoff";
import { deleteCache } from "../idb";
import EmailChange, { PROFILE, emailChangeEffects } from "./EmailChange";
import Frame from "./Frame";

const mount = document.getElementById("auth");
if (mount) {
	// Another link opened in this tab changes only the fragment, which its first script must read.
	window.addEventListener("hashchange", () => location.reload());
	const secret = takeEmailChange();
	const effects = emailChangeEffects(() => {
		// The console's saved responses still hold the email just replaced.
		deleteCache();
		location.replace(PROFILE);
	});
	handOff(mount, () =>
		render(
			() => (
				<Frame>
					<EmailChange secret={secret} effects={effects} />
				</Frame>
			),
			mount,
		),
	);
}
