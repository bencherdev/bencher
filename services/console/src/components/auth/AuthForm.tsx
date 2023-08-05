import { createSignal, createEffect, createMemo } from "solid-js";
// import axios from "axios";

// import Field, { FieldHandler } from "../field/Field";
// import AUTH_FIELDS from "./config/fields";
// import {
//     BENCHER_API_URL,
//     NotifyKind,
//     PLAN_PARAM,
//     post_options,
//     validate_jwt,
//     validate_plan_level,
// } from "../site/util";
// import { useLocation, useNavigate, useSearchParams } from "solid-app-router";
// import FieldKind from "../field/kind";
// import { notification_path } from "../site/Notification";
import { Email, JsonLogin, JsonSignup, PlanLevel } from "../../types/bencher";
import Field, { FieldHandler } from "../field/Field";
import FieldKind from "../field/kind";
import { useSearchParams } from "../../util/url";

export const INVITE_PARAM = "invite";
export const EMAIL_PARAM = "email";
export const TOKEN_PARAM = "token";

export interface Props {
    newUser: boolean;
    invite: Function;
}

type JsonAuthForm = JsonSignup | JsonLogin;

const AuthForm = (props: Props) => {
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
    const [form, setForm] = createSignal(initForm());

    const handleField: FieldHandler = (key, value, valid) => {
        setForm({
            ...form(),
            [key]: {
                value: value,
                valid: valid,
            },
        });
    };

    // const validateForm = () => {
    //     if (form()?.email?.valid) {
    //         if (props.newUser && form()?.username?.valid && form()?.consent?.value) {
    //             return true;
    //         }
    //         if (!props.newUser) {
    //             return true;
    //         }
    //     }
    //     return false;
    // };

    // const handleFormValid = () => {
    //     const valid = validateForm();
    //     if (valid !== form()?.valid) {
    //         setForm({ ...form(), valid: valid });
    //     }
    // };

    // const handleFormSubmitting = (submitting) => {
    //     setForm({ ...form(), submitting: submitting });
    // };

    // const post = async (data: JsonAuthForm) => {
    //     const url = `${BENCHER_API_URL()}/v0/auth/${props.newUser ? "signup" : "login"
    //         }`;
    //     const no_token = null;
    //     return await axios(post_options(url, no_token, data));
    // };

    const handleAuthFormSubmit = (event) => {
        //     event.preventDefault();
        //     handleFormSubmitting(true);
        //     const invite_token = props.invite();
        //     let invite: string | null;
        //     if (validate_jwt(invite_token)) {
        //         invite = invite_token;
        //     } else {
        //         invite = null;
        //     }

        //     let data: JsonAuthForm;
        //     let form_email: Email;
        //     if (props.newUser) {
        //         const signup_form = form();
        //         form_email = signup_form.email.value?.trim();
        //         data = {
        //             name: signup_form.username.value?.trim(),
        //             slug: null,
        //             email: form_email,
        //             plan: plan(),
        //             invite: invite,
        //         };

        //         if (!plan()) {
        //             setSearchParams({ [PLAN_PARAM]: PlanLevel.Free });
        //         }
        //     } else {
        //         const login_form = form();
        //         form_email = login_form.email.value?.trim();
        //         data = {
        //             email: form_email,
        //             plan: plan(),
        //             invite: invite,
        //         };
        //     }

        //     post(data)
        //         .then((_resp) => {
        //             handleFormSubmitting(false);
        //             navigate(
        //                 notification_path(
        //                     "/auth/confirm",
        //                     [PLAN_PARAM],
        //                     [[EMAIL_PARAM, form_email]],
        //                     NotifyKind.OK,
        //                     `Successful ${props.newUser ? "signup" : "login"
        //                     }! Please confirm token.`,
        //                 ),
        //             );
        //         })
        //         .catch((error) => {
        //             handleFormSubmitting(false);
        //             console.error(error);
        //             navigate(
        //                 notification_path(
        //                     pathname(),
        //                     [PLAN_PARAM, INVITE_PARAM],
        //                     [],
        //                     NotifyKind.ERROR,
        //                     `Failed to ${props.newUser ? "signup" : "login"
        //                     }. Please, try again.`,
        //                 ),
        //             );
        //         });
    };

    // createEffect(() => {
    //     handleFormValid();
    // });

    return (
        <form class="box">
            {props.newUser && (
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

            {props.newUser && form()?.username?.valid && form()?.email?.valid && (
                <Field
                    kind={FieldKind.CHECKBOX}
                    fieldKey="consent"
                    value={form()?.consent?.value}
                    valid={form()?.consent?.valid}
                    config={AUTH_FIELDS.consent}
                    handleField={handleField}
                />
            )}

            <div class="field">
                <p class="control">
                    <button
                        class="button is-primary is-fullwidth"
                        disabled={!form()?.valid || form()?.submitting}
                        onClick={handleAuthFormSubmit}
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

const AUTH_FIELDS = {
    username: {
        label: "Name",
        type: "text",
        placeholder: "Full Name",
        icon: "fas fa-user",
        help: "May only use: letters, numbers, contained spaces, apostrophes, periods, commas, and dashes",
        validate: (input: string) => validate_string(input, is_valid_user_name),
    },
    email: {
        label: "Email",
        type: "email",
        placeholder: "email@example.com",
        icon: "fas fa-envelope",
        help: "Must be a valid email address",
        // validate: (input) => validate_string(input, is_valid_email),
    },
    consent: {
        label: "I Agree",
        type: "checkbox",
        placeholder: (
            <small>
                {" "}
                I agree to the{" "}
                {
                    <a href="/legal/terms-of-use" target="_blank">
                        terms of use
                    </a>
                }
                ,{" "}
                {
                    <a href="/legal/privacy" target="_blank">
                        privacy policy
                    </a>
                }
                , and{" "}
                {
                    <a href="/legal/license" target="_blank">
                        license agreement
                    </a>
                }
                .
            </small>
        ),
    },
    token: {
        label: "Token",
        type: "text",
        placeholder: "jwt_header.jwt_payload.jwt_verify_signature",
        icon: "fas fa-key",
        help: "Must be a valid JWT (JSON Web Token)",
        // validate: validate_jwt,
    },
};

export default AuthForm;