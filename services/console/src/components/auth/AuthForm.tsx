import { createSignal, createEffect, createResource } from "solid-js";
import bencher_valid_init from "bencher_valid";

import {
	Email,
	JsonLogin,
	JsonSignup,
	AuthLogin,
	PlanLevel,
} from "../../types/bencher";
import Field, { FieldHandler } from "../field/Field";
import FieldKind from "../field/kind";
import { useNavigate, useSearchParams } from "../../util/url";
import { BENCHER_API_URL } from "../../util/ext";
import { httpPost } from "../../util/http";
import { AUTH_FIELDS } from "./auth";
import { createStore } from "solid-js/store";

export interface Props {
	newUser: boolean;
}

type JsonAuthForm = JsonSignup | JsonLogin;

const AuthForm = (props: Props) => {
	const [_bencher_valid] = createResource(
		async () => await bencher_valid_init(),
	);

	const navigate = useNavigate();
	const [searchParams, setSearchParams] = useSearchParams();
	// const navigate = useNavigate();
	// const location = useLocation();
	// const pathname = createMemo(() => location.pathname);
	// const [searchParams, setSearchParams] = useSearchParams();

	// if (!validate_plan_level(searchParams[PLAN_PARAM])) {
	//     setSearchParams({ [PLAN_PARAM]: null });
	// }
	// const plan = createMemo(() =>
	//     searchParams[PLAN_PARAM]
	//         ? (searchParams[PLAN_PARAM].trim() as PlanLevel)
	//         : null,
	// );
	const [form, setForm] = createStore(initForm());

	const handleField: FieldHandler = (key, value, valid) => {
		setForm({
			...form,
			[key]: {
				value: value,
				valid: valid,
			},
		});
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

	const handleFormValid = () => {
		const valid = validateForm();
		if (valid !== form?.valid) {
			setForm({ ...form, valid: valid });
		}
	};

	const handleFormSubmitting = (submitting: boolean) => {
		setForm({ ...form, submitting: submitting });
	};

	const post = async (data: JsonAuthForm) => {
		const url = `${BENCHER_API_URL()}/v0/auth/${
			props.newUser ? "signup" : "login"
		}`;
		const no_token = null;
		return await httpPost(url, no_token, data);
	};

	const handleSubmit = () => {
		handleFormSubmitting(true);
		// const invite_token = props.invite();
		//     let invite: string | null;
		//     if (validate_jwt(invite_token)) {
		//         invite = invite_token;
		//     } else {
		//         invite = null;
		//     }
		let plan = undefined;
		let invite = undefined;

		let auth_form: JsonAuthForm;
		if (props.newUser) {
			const signup_form = form;
			const signup: JsonSignup = {
				name: signup_form.username.value?.trim(),
				email: signup_form.email.value?.trim(),
			};
			auth_form = signup;
			if (!plan) {
				// setSearchParams({ [PLAN_PARAM]: PlanLevel.Free });
			}
		} else {
			const login_form = form;
			const login: AuthLogin = {
				email: login_form.email.value?.trim(),
			};
			auth_form = login;
		}
		if (plan) {
			auth_form.plan = plan;
		}
		if (invite) {
			auth_form.invite = invite;
		}

		post(auth_form)
			.then((_resp) => {
				handleFormSubmitting(false);
				navigate("/auth/confirm");
				// navigate(
				//     notification_path(
				//         "/auth/confirm",
				//         [PLAN_PARAM],
				//         [[EMAIL_PARAM, form_email]],
				//         NotifyKind.OK,
				//         `Successful ${props.newUser ? "signup" : "login"
				//         }! Please confirm token.`,
				//     ),
				// );
			})
			.catch((error) => {
				handleFormSubmitting(false);
				console.error(error);
				// navigate(
				//     notification_path(
				//         pathname(),
				//         [PLAN_PARAM, INVITE_PARAM],
				//         [],
				//         NotifyKind.ERROR,
				//         `Failed to ${props.newUser ? "signup" : "login"
				//         }. Please, try again.`,
				//     ),
				// );
			});
	};

	createEffect(() => {
		handleFormValid();
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
						disabled={!form?.valid || form?.submitting}
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
		valid: false,
		submitting: false,
	};
};

export default AuthForm;
