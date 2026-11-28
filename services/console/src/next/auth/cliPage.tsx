import { render } from "solid-js/web";
import { keepOutOfReports } from "../../util/scrub";
import { takeAuthReturn } from "../astro/authReturn";
import { handOff } from "../handoff";
import CliCode from "./CliCode";
import Frame from "./Frame";

const boot = (mount: HTMLElement) => {
	// The page's first script took the code from the address; a denied or
	// refused request carries none.
	const { code } = takeAuthReturn();
	keepOutOfReports(code ? [code] : []);
	handOff(mount, () =>
		render(
			() => (
				<Frame>
					<CliCode
						code={code}
						// The consent page's own state for a link that is not valid.
						malformed={() => location.replace("/auth/authorize")}
					/>
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
