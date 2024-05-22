import { Show } from "solid-js";
import { authUser } from "../../util/auth";
import { BENCHER_GITHUB_URL } from "../../util/ext";

const GitHubLink = () => {
	return (
		<Show
			when={authUser()?.token}
			fallback={
				<a
					class="navbar-item"
					href={BENCHER_GITHUB_URL}
					target="_blank"
					rel="noreferrer"
				>
					GitHub
				</a>
			}
		>
			<div />
		</Show>
	);
};

export default GitHubLink;
