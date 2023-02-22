import {
	createSignal,
	createEffect,
	createMemo,
	Switch,
	Match,
} from "solid-js";
import axios from "axios";

import Field from "../field/Field";
import AUTH_FIELDS from "./config/fields";
import { FormKind } from "./config/types";
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

export const INVITE_PARAM = "invite";
export const EMAIL_PARAM = "email";
export const TOKEN_PARAM = "token";

export interface Props {
	config: any;
	invite: Function;
	user: Function;
	handleUser: Function;
}

export const AuthForm = (props: Props) => {
	const navigate = useNavigate();
	const location = useLocation();
	const pathname = createMemo(() => location.pathname);
	const [searchParams, setSearchParams] = useSearchParams();

	if (
		searchParams[PLAN_PARAM] &&
		!validate_plan_level(searchParams[PLAN_PARAM])
	) {
		setSearchParams({ [PLAN_PARAM]: null });
	}
	const plan = createMemo(() =>
		searchParams[PLAN_PARAM] ? searchParams[PLAN_PARAM].trim() : null,
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
			if (props.config?.kind === FormKind.LOGIN) {
				return true;
			}
			if (
				props.config?.kind === FormKind.SIGNUP &&
				form()?.username?.valid &&
				form()?.consent?.value
			) {
				return true;
			}
		}
		return false;
	};

	const handleFormValid = () => {
		var valid = validateForm();
		if (valid !== form()?.valid) {
			setForm({ ...form(), valid: valid });
		}
	};

	const handleFormSubmitting = (submitting) => {
		setForm({ ...form(), submitting: submitting });
	};

	const post = async (data: {
		name: null | string;
		slug: null | string;
		email: string;
		invite: null | string;
	}) => {
		const url = `${BENCHER_API_URL()}/v0/auth/${props.config?.kind}`;
		const no_token = null;
		let resp = await axios(post_options(url, no_token, data));
		return resp.data;
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

		let data;
		let form_email;
		if (props.config?.kind === FormKind.SIGNUP) {
			const signup_form = form();
			form_email = signup_form.email.value?.trim();
			data = {
				name: signup_form.username.value?.trim(),
				slug: null,
				email: form_email,
				plan: plan(),
				invite: invite,
			};
		} else if (props.config?.kind === FormKind.LOGIN) {
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
				navigate(
					notification_path(
						props.config?.redirect,
						[PLAN_PARAM],
						[[EMAIL_PARAM, form_email]],
						NotifyKind.OK,
						`Successful ${props.config?.kind} please confirm token.`,
					),
				);
			})
			.catch((_e) => {
				navigate(
					notification_path(
						pathname(),
						[PLAN_PARAM, INVITE_PARAM],
						[],
						NotifyKind.ERROR,
						`Failed to ${props.config?.kind} please try again.`,
					),
				);
			});

		handleFormSubmitting(false);
	};

	createEffect(() => {
		handleFormValid();
	});

	return (
		<form class="box">
			{props.config?.kind === FormKind.SIGNUP && (
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

			{props.config?.kind === FormKind.SIGNUP &&
				form()?.username?.valid &&
				form()?.email?.valid && (
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
				<Switch fallback={<>Submit</>}>
					<Match when={props.config?.kind === FormKind.SIGNUP}>
						<>Sign up</>
					</Match>
					<Match when={props.config?.kind === FormKind.LOGIN}>
						<>Log in</>
					</Match>
				</Switch>
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
