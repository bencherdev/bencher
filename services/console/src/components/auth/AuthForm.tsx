import {
	createEffect,
	createResource,
	createSignal,
	createMemo,
} from "solid-js";
import bencher_valid_init from "bencher_valid";

import { JsonLogin, JsonSignup, PlanLevel, Jwt } from "../../types/bencher";
import Field, { FieldHandler } from "../field/Field";
import FieldKind from "../field/kind";
import { useSearchParams } from "../../util/url";
import { httpPost } from "../../util/http";
import { AUTH_FIELDS, EMAIL_PARAM, INVITE_PARAM, PLAN_PARAM } from "./auth";
import { createStore } from "solid-js/store";
import { validJwt, validPlanLevel } from "../../util/valid";
import { NotifyKind, navigateNotify, pageNotify } from "../../util/notify";

export interface Props {
	apiUrl: string;
	newUser: boolean;
}

type JsonAuthForm = JsonSignup | JsonLogin;

const AuthForm = (props: Props) => {
	const [bencher_valid] = createResource(
		async () => await bencher_valid_init(),
	);
	const [searchParams, setSearchParams] = useSearchParams();

	const apiUrl = createMemo(() => props.apiUrl);

	const plan = () => searchParams[PLAN_PARAM]?.trim() as PlanLevel;
	const invite = () => searchParams[INVITE_PARAM]?.trim() as Jwt;
	const [form, setForm] = createStore(initForm());
	const [submitting, setSubmitting] = createSignal(false);
	const [valid, setValid] = createSignal(false);

	const isSendable = (): boolean => {
		return !submitting() && valid();
	};

	const handleField: FieldHandler = (key, value, valid) => {
		setForm({
			...form,
			[key]: {
				value: value,
				valid: valid,
			},
		});

		setValid(validateForm());
	};

	const validateForm = () => {
		if (form?.email?.valid) {
			if (props.newUser && form?.username?.valid && form?.consent?.value) {
				return true;
			}
			if (!props.newUser) {
				return true;
			}
		}
		return false;
	};

	const handleSubmit = () => {
		setSubmitting(true);
		const plan_level = plan();
		const invite_token = invite();

		let authForm: JsonAuthForm;
		if (props.newUser) {
			const signup_form = form;
			const signup: JsonSignup = {
				name: signup_form.username.value?.trim(),
				email: signup_form.email.value?.trim(),
			};
			authForm = signup;
			if (!plan_level) {
				setSearchParams({ [PLAN_PARAM]: PlanLevel.Free });
			}
		} else {
			const login_form = form;
			const login: JsonLogin = {
				email: login_form.email.value?.trim(),
			};
			authForm = login;
		}
		if (plan_level) {
			authForm.plan = plan_level;
		}
		if (invite_token) {
			authForm.invite = invite_token;
		}

		httpPost(
			apiUrl(),
			`/v0/auth/${props.newUser ? "signup" : "login"}`,
			null,
			authForm,
		)
			.then((_resp) => {
				setSubmitting(false);
				navigateNotify(
					NotifyKind.OK,
					`Successful ${
						props.newUser ? "signup" : "login"
					}! Please confirm token.`,
					"/auth/confirm",
					[PLAN_PARAM],
					[[EMAIL_PARAM, authForm.email]],
				);
			})
			.catch((error) => {
				setSubmitting(false);
				console.error(error);
				pageNotify(
					NotifyKind.ERROR,
					`Failed to ${props.newUser ? "signup" : "login"}. Please, try again.`,
				);
			});
	};

	createEffect(() => {
		if (!bencher_valid()) {
			return;
		}

		const newParams: Record<string, null> = {};
		if (!validPlanLevel(searchParams[PLAN_PARAM])) {
			newParams[PLAN_PARAM] = null;
		}
		if (!validJwt(searchParams[INVITE_PARAM])) {
			newParams[INVITE_PARAM] = null;
		}
		if (Object.keys(newParams).length !== 0) {
			setSearchParams(newParams);
		}
	});

	return (
		<form class="box">
			{props.newUser && (
				<Field
					kind={FieldKind.INPUT}
					fieldKey="username"
					label="Name"
					value={form?.username?.value}
					valid={form?.username?.valid}
					config={AUTH_FIELDS.username}
					handleField={handleField}
				/>
			)}

			<Field
				kind={FieldKind.INPUT}
				fieldKey="email"
				label="Email"
				value={form?.email?.value}
				valid={form?.email?.valid}
				config={AUTH_FIELDS.email}
				handleField={handleField}
			/>

			<br />

			{props.newUser && form?.username?.valid && form?.email?.valid && (
				<Field
					kind={FieldKind.CHECKBOX}
					fieldKey="consent"
					value={form?.consent?.value}
					valid={form?.consent?.valid}
					config={AUTH_FIELDS.consent}
					handleField={handleField}
				/>
			)}

			<div class="field">
				<p class="control">
					<button
						class="button is-primary is-fullwidth"
						disabled={!isSendable()}
						onClick={(e) => {
							e.preventDefault();
							handleSubmit();
						}}
					>
						{props.newUser ? "Sign up" : "Log in"}
					</button>
				</p>
			</div>
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
	};
};

export default AuthForm;
