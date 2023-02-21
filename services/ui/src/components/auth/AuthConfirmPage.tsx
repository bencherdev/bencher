import axios from "axios";
import { useLocation, useNavigate, useSearchParams } from "solid-app-router";
import { createEffect, createMemo, createSignal } from "solid-js";
import AUTH_FIELDS from "./config/fields";
import Field from "../field/Field";
import {
	BENCHER_API_URL,
	NotifyKind,
	pageTitle,
	post_options,
	validate_jwt,
	validate_plan_level,
} from "../site/util";
import Notification, { notification_path } from "../site/Notification";
import FieldKind from "../field/kind";
import { EMAIL_PARAM, PLAN_PARAM, TOKEN_PARAM } from "./AuthForm";
import { PlanLevel } from "../console/panel/billing/Pricing";

const CONFIRM_FORWARD = [EMAIL_PARAM, TOKEN_PARAM, PLAN_PARAM];

const AuthConfirmPage = (props: {
	config: any;
	user: any;
	handleUser: Function;
}) => {
	const navigate = useNavigate();
	const location = useLocation();
	const pathname = createMemo(() => location.pathname);
	const [searchParams, setSearchParams] = useSearchParams();

	if (searchParams[TOKEN_PARAM] && !validate_jwt(searchParams[TOKEN_PARAM])) {
		setSearchParams({ [TOKEN_PARAM]: null });
	}
	const token = createMemo(() =>
		searchParams[TOKEN_PARAM] ? searchParams[TOKEN_PARAM].trim() : null,
	);

	if (!validate_plan_level(searchParams[PLAN_PARAM])) {
		setSearchParams({ [PLAN_PARAM]: PlanLevel.FREE });
	}
	const plan = createMemo(() =>
		searchParams[PLAN_PARAM] ? searchParams[PLAN_PARAM].trim() : null,
	);

	const email = createMemo(() =>
		searchParams[EMAIL_PARAM] ? searchParams[EMAIL_PARAM].trim() : null,
	);

	const [submitted, setSubmitted] = createSignal();
	const [form, setForm] = createSignal(initForm());

	const [cool_down, setCoolDown] = createSignal(true);
	setTimeout(() => setCoolDown(false), 10000);

	const handleField = (key, value, valid) => {
		setForm({
			...form(),
			[key]: {
				value: value,
				valid: valid,
			},
		});
	};

	const post = async () => {
		const url = props.config?.form?.path;
		const no_token = null;
		const data = {
			token: token(),
		};
		const resp = await axios(post_options(url, no_token, data));
		return resp.data;
	};

	const handleFormSubmit = () => {
		handleFormSubmitting(true);
		post()
			.then((data) => {
				if (!props.handleUser(data)) {
					navigate(
						notification_path(
							pathname(),
							CONFIRM_FORWARD,
							[],
							NotifyKind.ERROR,
							"Invalid user please try again.",
						),
					);
				}
			})
			.catch((e) => {
				navigate(
					notification_path(
						pathname(),
						CONFIRM_FORWARD,
						[],
						NotifyKind.ERROR,
						"Failed to confirm token please try again.",
					),
				);
			});
		handleFormSubmitting(false);
	};

	const handleFormSubmitting = (submitting) => {
		setForm({ ...form(), submitting: submitting });
	};

	const post_resend = async (data: {
		email: string;
		plan: null | string;
	}) => {
		const url = `${BENCHER_API_URL()}/v0/auth/login`;
		const no_token = null;
		let resp = await axios(post_options(url, no_token, data));
		return resp.data;
	};

	const handleResendEmail = (event) => {
		event.preventDefault();
		handleFormSubmitting(true);

		const data = {
			email: email().trim(),
			plan: plan()?.trim(),
		};

		post_resend(data)
			.then((_resp) => {
				navigate(
					notification_path(
						pathname(),
						CONFIRM_FORWARD,
						[],
						NotifyKind.OK,
						`Successful resent email to ${email()} please confirm token.`,
					),
				);
			})
			.catch((_e) => {
				navigate(
					notification_path(
						pathname(),
						CONFIRM_FORWARD,
						[],
						NotifyKind.ERROR,
						`Failed to resend email to ${email()} please try again.`,
					),
				);
			});

		handleFormSubmitting(false);
		setCoolDown(true);
		setTimeout(() => setCoolDown(false), 30000);
	};

	createEffect(() => {
		pageTitle(props.config?.title);

		if (validate_jwt(props.user?.token)) {
			navigate(
				notification_path(
					props.config?.form?.redirect[plan() ? plan() : "free"],
					[PLAN_PARAM],
					[],
					NotifyKind.OK,
					"Ahoy!",
				),
			);
		}

		const value = form()?.token?.value;
		if (value.length > 0) {
			setSearchParams({ [TOKEN_PARAM]: value });
		}

		const valid = form()?.token?.valid;
		if (valid !== form()?.valid) {
			setForm({ ...form(), valid: valid });
		}

		const jwt = token();
		if (validate_jwt(jwt) && jwt !== submitted()) {
			setSubmitted(jwt);
			handleFormSubmit();
		}
	});

	return (
		<>
			<Notification />

			<section class="section">
				<div class="container">
					<div class="columns is-centered">
						<div class="column is-two-fifths">
							<h2 class="title">{props.config?.title}</h2>
							<h3 class="subtitle">{props.config?.sub}</h3>

							<form class="box">
								<Field
									kind={FieldKind.INPUT}
									fieldKey="token"
									label={true}
									value={form()?.token?.value}
									valid={form()?.token?.valid}
									config={AUTH_FIELDS.token}
									handleField={handleField}
								/>

								<button
									class="button is-primary is-fullwidth"
									disabled={!form()?.valid || form()?.submitting}
									onClick={(e) => {
										e.preventDefault();
										handleFormSubmit();
									}}
								>
									Submit
								</button>
							</form>

							{email() && (
								<>
									<hr />

									<div class="content has-text-centered">
										<button
											class="button is-small is-black is-inverted"
											disabled={form()?.submitting || cool_down()}
											onClick={handleResendEmail}
										>
											<div>Click to resend email to: {email()}</div>
										</button>
									</div>
								</>
							)}
						</div>
					</div>
				</div>
			</section>
		</>
	);
};

const initForm = () => {
	return {
		token: {
			value: "",
			valid: null,
		},
		valid: false,
		submitting: false,
	};
};

export default AuthConfirmPage;
