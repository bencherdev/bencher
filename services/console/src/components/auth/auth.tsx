import { validEmail, validJwt, validUserName } from "../../util/valid";

export const INVITE_PARAM = "invite";
export const EMAIL_PARAM = "email";
export const TOKEN_PARAM = "token";

export const AUTH_FIELDS = {
    username: {
        label: "Name",
        type: "text",
        placeholder: "Full Name",
        icon: "fas fa-user",
        help: "May only use: letters, numbers, contained spaces, apostrophes, periods, commas, and dashes",
        validate: validUserName,
    },
    email: {
        label: "Email",
        type: "email",
        placeholder: "email@example.com",
        icon: "fas fa-envelope",
        help: "Must be a valid email address",
        validate: validEmail,
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
        validate: validJwt,
    },
};