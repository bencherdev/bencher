import { decodePath } from "../../util/url";
import AuthOverride from "../auth/AuthOverride";
import { BENCHER_NAVBAR_AUTH_ID } from "./id";

const DocsNavbar = () => {
	return (
		<AuthOverride elementId={BENCHER_NAVBAR_AUTH_ID}>
			<div id={BENCHER_NAVBAR_AUTH_ID}>
				<a class="button" href={decodePath("/console")}>
					<span class="icon has-text-primary">
						<i class="fas fa-angle-left" aria-hidden="true" />
					</span>
					<span>Back to Console</span>
				</a>
			</div>
		</AuthOverride>
	);
};

export default DocsNavbar;
