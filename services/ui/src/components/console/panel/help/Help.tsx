const Help = (props) => {
	return (
		<section class="section">
			<div class="container">
				<div class="columns">
					<div class="column">
						<h4 class="title">Hey {props.user?.user?.name}!</h4>
						<h4 class="subtitle">There are many ways to get help</h4>
						<br />

						<h4 class="title">GitHub</h4>
						<h4 class="subtitle">
							<a
								href="https://github.com/bencherdev/bencher/issues"
								target="_blank"
								rel="noreferrer"
							>
								Open an issue on GitHub
							</a>
						</h4>
						<br />

						<h4 class="title">Discord</h4>
						<h4 class="subtitle">
							<a
								href="https://discord.gg/yGEsdUh7R4"
								target="_blank"
								rel="noreferrer"
							>
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
