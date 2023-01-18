import { Link, useNavigate, useSearchParams } from "solid-app-router";
import { createEffect, createMemo } from "solid-js";

import { pageTitle, validate_jwt } from "../site/util";
import { AuthForm } from "./AuthForm";
import { Auth } from "./config/types";
import Notification from "../site/Notification";

const INVITE_PARAM = "invite";

const AuthFormPage = (props: {
	config: any;
	user: Function;
	handleUser: Function;
}) => {
	const navigate = useNavigate();
	const [searchParams, setSearchParams] = useSearchParams();

	if (searchParams[INVITE_PARAM] && !validate_jwt(searchParams[INVITE_PARAM])) {
		setSearchParams({ [INVITE_PARAM]: null });
	}

	const invite = createMemo(() =>
		searchParams[INVITE_PARAM] ? searchParams[INVITE_PARAM].trim() : null,
	);

	createEffect(() => {
		if (validate_jwt(props.user().token)) {
			navigate("/console");
		}

		pageTitle(props.config?.title);
	});

	return (
		<>
			<Notification />

			<section class="section">
				<div class="container">
					<div class="columns is-centered">
						<div class="column is-two-fifths">
							<h2 class="title">{props.config?.title}</h2>

							<AuthForm
								config={props.config?.form}
								user={props.user}
								invite={invite}
								handleUser={props.handleUser}
							/>

							<hr />

							<p class="has-text-centered">
								<small>
									switch to{" "}
									{props.config?.auth === Auth.SIGNUP && (
										<Link href="/auth/login">log in</Link>
									)}
									{props.config?.auth === Auth.LOGIN && (
										<Link href="/auth/signup">sign up</Link>
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
