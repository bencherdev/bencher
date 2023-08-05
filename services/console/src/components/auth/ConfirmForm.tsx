import bencher_valid_init from "bencher_valid";
import { createResource, createSignal } from "solid-js";

import type { FieldHandler } from "../field/Field";
import Field from "../field/Field";
import FieldKind from "../field/kind";
import { AUTH_FIELDS } from "./auth";

// import axios from "axios";
// import { useLocation, useNavigate, useSearchParams } from "solid-app-router";
// import { createEffect, createMemo, createSignal } from "solid-js";
// import AUTH_FIELDS from "./config/fields";
// import Field, { FieldHandler } from "../field/Field";
// import {
//     BENCHER_API_URL,
//     NotifyKind,
//     pageTitle,
//     PLAN_PARAM,
//     post_options,
//     validate_jwt,
//     validate_plan_level,
// } from "../site/util";
// import Notification, { notification_path } from "../site/Notification";
// import FieldKind from "../field/kind";
// import { EMAIL_PARAM, TOKEN_PARAM } from "./AuthForm";
// import { JsonConfirm } from "../../types/bencher";

// const CONFIRM_FORWARD = [EMAIL_PARAM, TOKEN_PARAM, PLAN_PARAM];

export interface Props { }

const ConfirmForm = (_props: Props) => {
    const [_bencher_valid] = createResource(async () => await bencher_valid_init());

    // const navigate = useNavigate();
    // const location = useLocation();
    // const pathname = createMemo(() => location.pathname);
    // const [searchParams, setSearchParams] = useSearchParams();

    // if (searchParams[TOKEN_PARAM] && !validate_jwt(searchParams[TOKEN_PARAM])) {
    //     setSearchParams({ [TOKEN_PARAM]: null });
    // }
    // const token = createMemo(() =>
    //     searchParams[TOKEN_PARAM] ? searchParams[TOKEN_PARAM].trim() : null,
    // );

    // if (!validate_plan_level(searchParams[PLAN_PARAM])) {
    //     setSearchParams({ [PLAN_PARAM]: null });
    // }
    // const plan = createMemo(() =>
    //     searchParams[PLAN_PARAM] ? searchParams[PLAN_PARAM].trim() : null,
    // );

    // const email = createMemo(() =>
    //     searchParams[EMAIL_PARAM] ? searchParams[EMAIL_PARAM].trim() : null,
    // );

    // const title = "Confirm Token";

    // const [submitted, setSubmitted] = createSignal();
    const [form, setForm] = createSignal(initForm());

    // const [cool_down, setCoolDown] = createSignal(true);
    // setTimeout(() => setCoolDown(false), 10000);

    const handleField: FieldHandler = (key, value, valid) => {
        setForm({
            ...form(),
            [key]: {
                value: value,
                valid: valid,
            },
        });
    };

    // const post = async () => {
    //     const url = `${BENCHER_API_URL()}/v0/auth/confirm`;
    //     const no_token = null;
    //     const data = {
    //         token: token(),
    //     };
    //     return await axios(post_options(url, no_token, data));
    // };

    // const handleFormSubmit = () => {
    //     handleFormSubmitting(true);

    //     post()
    //         .then((resp) => {
    //             handleFormSubmitting(false);
    //             if (!props.handleUser(resp?.data)) {
    //                 navigate(
    //                     notification_path(
    //                         pathname(),
    //                         CONFIRM_FORWARD,
    //                         [],
    //                         NotifyKind.ERROR,
    //                         "Invalid user. Please, try again.",
    //                     ),
    //                 );
    //             }
    //         })
    //         .catch((error) => {
    //             handleFormSubmitting(false);
    //             console.error(error);
    //             navigate(
    //                 notification_path(
    //                     pathname(),
    //                     CONFIRM_FORWARD,
    //                     [],
    //                     NotifyKind.ERROR,
    //                     "Failed to confirm token. Please, try again.",
    //                 ),
    //             );
    //         });
    // };

    // const handleFormSubmitting = (submitting) => {
    //     setForm({ ...form(), submitting: submitting });
    // };

    // const post_resend = async (data: {
    //     email: string;
    //     plan: null | string;
    // }) => {
    //     const url = `${BENCHER_API_URL()}/v0/auth/login`;
    //     const no_token = null;
    //     return await axios(post_options(url, no_token, data));
    // };

    // const handleResendEmail = (event) => {
    //     event.preventDefault();
    //     handleFormSubmitting(true);

    //     const data = {
    //         email: email().trim(),
    //         plan: plan()?.trim(),
    //     };

    //     post_resend(data)
    //         .then((_resp) => {
    //             handleFormSubmitting(false);
    //             navigate(
    //                 notification_path(
    //                     pathname(),
    //                     CONFIRM_FORWARD,
    //                     [],
    //                     NotifyKind.OK,
    //                     `Successful resent email to ${email()} please confirm token.`,
    //                 ),
    //             );
    //         })
    //         .catch((error) => {
    //             handleFormSubmitting(false);
    //             console.error(error);
    //             navigate(
    //                 notification_path(
    //                     pathname(),
    //                     CONFIRM_FORWARD,
    //                     [],
    //                     NotifyKind.ERROR,
    //                     `Failed to resend email to ${email()}. Please, try again.`,
    //                 ),
    //             );
    //         });

    //     setCoolDown(true);
    //     setTimeout(() => setCoolDown(false), 30000);
    // };

    // createEffect(() => {
    //     pageTitle(title);

    //     if (validate_jwt(props.user?.token)) {
    //         navigate(
    //             notification_path(
    //                 {
    //                     free: "/console",
    //                     team: "/console/billing",
    //                     enterprise: "/console/billing",
    //                 }[plan() ? plan() : "free"],
    //                 [PLAN_PARAM],
    //                 [],
    //                 NotifyKind.OK,
    //                 "Ahoy!",
    //             ),
    //         );
    //     }

    //     const value = form()?.token?.value;
    //     if (value.length > 0) {
    //         setSearchParams({ [TOKEN_PARAM]: value });
    //     }

    //     const valid = form()?.token?.valid;
    //     if (valid !== form()?.valid) {
    //         setForm({ ...form(), valid: valid });
    //     }

    //     const jwt = token();
    //     if (validate_jwt(jwt) && jwt !== submitted()) {
    //         setSubmitted(jwt);
    //         handleFormSubmit();
    //     }
    // });

    return (
        <>

            <form class="box">
                <Field
                    kind={FieldKind.INPUT}
                    fieldKey="token"
                    value={form()?.token?.value}
                    valid={form()?.token?.valid}
                    config={AUTH_FIELDS.token}
                    handleField={handleField}
                />

                <div class="field">
                    <p class="control">
                        <button
                            class="button is-primary is-fullwidth"
                            disabled={!form()?.valid || form()?.submitting}
                            onClick={(e) => {
                                e.preventDefault();
                                // handleFormSubmit();
                            }}
                        >
                            Submit
                        </button>
                    </p>
                </div>
            </form>

            {/* {email() && (
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
            )} */}
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

export default ConfirmForm;
