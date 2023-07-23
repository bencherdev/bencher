import { Link, useNavigate, useSearchParams } from "solid-app-router";
import { createEffect, createMemo } from "solid-js";

import { pageTitle, validate_jwt } from "../site/util";
import { AuthForm } from "./AuthForm";
import Notification from "../site/Notification";
import { JsonConfirm } from "../../types/bencher";

const INVITE_PARAM = "invite";

const AuthFormPage = (props: {
	new_user: boolean;
	user: JsonConfirm;
}) => {
	const navigate = useNavigate();
	const [searchParams, setSearchParams] = useSearchParams();

	if (searchParams[INVITE_PARAM] && !validate_jwt(searchParams[INVITE_PARAM])) {
		setSearchParams({ [INVITE_PARAM]: null });
	}
	const invite = createMemo(() =>
		searchParams[INVITE_PARAM] ? searchParams[INVITE_PARAM].trim() : null,
	);

	const title = props.new_user ? "Sign up" : "Log in";

	createEffect(() => {
		if (validate_jwt(props.user?.token)) {
			navigate("/console");
		}

		pageTitle(title);
	});

	return (
		<>
			<Notification />

			<section class="section">
				<div class="container">
					<div class="columns is-centered">
						<div class="column is-two-fifths">
							<h2 class="title">{title}</h2>

							<AuthForm new_user={props.new_user} invite={invite} />

							<hr />

							<p class="has-text-centered">
								<small>
									switch to{" "}
									{props.new_user ? (
										<Link title="Switch to Log in" href="/auth/login">
											log in
										</Link>
									) : (
										<Link title="Switch to Sign up" href="/auth/signup">
											sign up
										</Link>
									)}
								</small>
							</p>
						</div>
					</div>
				</div>
			</section>
		</>
	);
};

export default AuthFormPage;
