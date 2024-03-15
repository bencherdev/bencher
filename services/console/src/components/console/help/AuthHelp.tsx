import { authUser } from "../../../util/auth";
import { BENCHER_CHAT_URL, BENCHER_GITHUB_URL } from "../../../util/ext";

const AuthHelp = () => {
	const user = authUser();

	return (
		<section class="section">
			<div class="container">
				<div class="columns">
					<div class="column">
						<h4 class="title is-4">
							Ahoy{user?.user?.name && ` ${user?.user?.name}`}!
						</h4>
						<h4 class="subtitle is-4">There are many ways to get help</h4>
						<br />

						<h4 class="title is-4">Bencher Docs</h4>
						<h4 class="subtitle is-4">
							<a href="/docs/">Read the docs</a>
						</h4>
						<br />

						<h4 class="title is-4">Bencher API Docs</h4>
						<h4 class="subtitle is-4">
							<a href="/docs/api/">Read the docs</a>
						</h4>
						<br />

						<h4 class="title is-4">GitHub</h4>
						<h4 class="subtitle is-4">
							<a
								href={`${BENCHER_GITHUB_URL}/issues`}
								target="_blank"
								rel="noreferrer"
							>
								Open an issue on GitHub
							</a>
						</h4>
						<br />

						<h4 class="title is-4">Discord</h4>
						<h4 class="subtitle is-4">
							<a href={BENCHER_CHAT_URL} target="_blank" rel="noreferrer">
								Join the chat
							</a>
						</h4>
						<br />

						<h4 class="title is-4">Email</h4>
						<h4 class="subtitle is-4">
							Email us directly at{" "}
							<a href="mailto:help@bencher.dev">help@bencher.dev</a>
						</h4>
						<br />
					</div>
				</div>
			</div>
		</section>
	);
};

export default AuthHelp;
