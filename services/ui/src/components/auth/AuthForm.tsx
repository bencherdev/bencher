import { createSignal, createEffect, createMemo } from "solid-js";
import axios from "axios";

import Field from "../field/Field";
import AUTH_FIELDS from "./config/fields";
import {
	BENCHER_API_URL,
	NotifyKind,
	PLAN_PARAM,
	post_options,
	validate_jwt,
	validate_plan_level,
} from "../site/util";
import { useLocation, useNavigate, useSearchParams } from "solid-app-router";
import FieldKind from "../field/kind";
import { notification_path } from "../site/Notification";
import { Email, JsonLogin, JsonSignup, PlanLevel } from "../../types/bencher";

export const INVITE_PARAM = "invite";
export const EMAIL_PARAM = "email";
export const TOKEN_PARAM = "token";

export interface Props {
	new_user: boolean;
	invite: Function;
}

type JsonAuthForm = JsonSignup | JsonLogin;

export const AuthForm = (props: Props) => {
	const navigate = useNavigate();
	const location = useLocation();
	const pathname = createMemo(() => location.pathname);
	const [searchParams, setSearchParams] = useSearchParams();

	if (!validate_plan_level(searchParams[PLAN_PARAM])) {
		setSearchParams({ [PLAN_PARAM]: null });
	}
	const plan = createMemo(() =>
		searchParams[PLAN_PARAM]
			? (searchParams[PLAN_PARAM].trim() as PlanLevel)
			: null,
	);
	const [form, setForm] = createSignal(initForm());

	const handleField = (key, value, valid) => {
		setForm({
			...form(),
			[key]: {
				value: value,
				valid: valid,
			},
		});
	};

	const validateForm = () => {
		if (form()?.email?.valid) {
			if (props.new_user && form()?.username?.valid && form()?.consent?.value) {
				return true;
			}
			if (!props.new_user) {
				return true;
			}
		}
		return false;
	};

	const handleFormValid = () => {
		const valid = validateForm();
		if (valid !== form()?.valid) {
			setForm({ ...form(), valid: valid });
		}
	};

	const handleFormSubmitting = (submitting) => {
		setForm({ ...form(), submitting: submitting });
	};

	const post = async (data: JsonAuthForm) => {
		const url = `${BENCHER_API_URL()}/v0/auth/${
			props.new_user ? "signup" : "login"
		}`;
		const no_token = null;
		return await axios(post_options(url, no_token, data));
	};

	const handleAuthFormSubmit = (event) => {
		event.preventDefault();
		handleFormSubmitting(true);
		const invite_token = props.invite();
		let invite: string | null;
		if (validate_jwt(invite_token)) {
			invite = invite_token;
		} else {
			invite = null;
		}

		let data: JsonAuthForm;
		let form_email: Email;
		if (props.new_user) {
			const signup_form = form();
			form_email = signup_form.email.value?.trim();
			data = {
				name: signup_form.username.value?.trim(),
				slug: null,
				email: form_email,
				plan: plan(),
				invite: invite,
			};

			if (!plan()) {
				setSearchParams({ [PLAN_PARAM]: PlanLevel.Free });
			}
		} else {
			const login_form = form();
			form_email = login_form.email.value?.trim();
			data = {
				email: form_email,
				plan: plan(),
				invite: invite,
			};
		}

		post(data)
			.then((_resp) => {
				handleFormSubmitting(false);
				navigate(
					notification_path(
						"/auth/confirm",
						[PLAN_PARAM],
						[[EMAIL_PARAM, form_email]],
						NotifyKind.OK,
						`Successful ${
							props.new_user ? "signup" : "login"
						}! Please confirm token.`,
					),
				);
			})
			.catch((error) => {
				handleFormSubmitting(false);
				console.error(error);
				navigate(
					notification_path(
						pathname(),
						[PLAN_PARAM, INVITE_PARAM],
						[],
						NotifyKind.ERROR,
						`Failed to ${
							props.new_user ? "signup" : "login"
						}. Please, try again.`,
					),
				);
			});
	};

	createEffect(() => {
		handleFormValid();
	});

	return (
		<form class="box">
			{props.new_user && (
				<Field
					kind={FieldKind.INPUT}
					fieldKey="username"
					label="Name"
					value={form()?.username?.value}
					valid={form()?.username?.valid}
					config={AUTH_FIELDS.username}
					handleField={handleField}
				/>
			)}

			<Field
				kind={FieldKind.INPUT}
				fieldKey="email"
				label="Email"
				value={form()?.email?.value}
				valid={form()?.email?.valid}
				config={AUTH_FIELDS.email}
				handleField={handleField}
			/>

			<br />

			{props.new_user && form()?.username?.valid && form()?.email?.valid && (
				<Field
					kind={FieldKind.CHECKBOX}
					fieldKey="consent"
					label=""
					value={form()?.consent?.value}
					valid={form()?.consent?.valid}
					config={AUTH_FIELDS.consent}
					handleField={handleField}
				/>
			)}

			<button
				class="button is-primary is-fullwidth"
				disabled={!form()?.valid || form()?.submitting}
				onClick={handleAuthFormSubmit}
			>
				{props.new_user ? "Sign up" : "Log in"}
			</button>
		</form>
	);
};

const initForm = () => {
	return {
		username: {
			value: "",
			valid: null,
		},
		email: {
			value: "",
			valid: null,
		},
		consent: {
			value: false,
			valid: null,
		},
		valid: false,
		submitting: false,
	};
};
