import { BENCHER_CHAT_URL, BENCHER_GITHUB_URL } from "../../../site/util";

const Help = (props) => {
	return (
		<section class="section">
			<div class="container">
				<div class="columns">
					<div class="column">
						<h4 class="title">
							Hey{props.user?.user?.name && ` ${props.user?.user?.name}`}!
						</h4>
						<h4 class="subtitle">There are many ways to get help</h4>
						<br />

						<h4 class="title">GitHub</h4>
						<h4 class="subtitle">
							<a
								href={`${BENCHER_GITHUB_URL}/issues`}
								target="_blank"
								rel="noreferrer"
							>
								Open an issue on GitHub
							</a>
						</h4>
						<br />

						<h4 class="title">Discord</h4>
						<h4 class="subtitle">
							<a href={BENCHER_CHAT_URL} target="_blank" rel="noreferrer">
								Join the chat
							</a>
						</h4>
						<br />

						<h4 class="title">Email</h4>
						<h4 class="subtitle">
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

export default Help;
