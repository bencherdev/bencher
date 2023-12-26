interface Props {
	newUser: boolean;
	githubClientId: string;
}

const OAuthForm = (props: Props) => {
	if (props.githubClientId) {
		return (
			<>
				<div class="is-divider" data-content="OR"></div>
				<a
					class="button is-fullwidth is-outlined"
					href={`https://github.com/login/oauth/authorize?client_id=${props.githubClientId}`}
				>
					<span class="icon">
						<i class="fab fa-github" aria-hidden="true" />
					</span>
					<span>{props.newUser ? "Sign up" : "Sign in"} with GitHub</span>
				</a>
			</>
		);
	} else {
		return <></>;
	}
};

export default OAuthForm;
