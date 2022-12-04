import { is_valid_email, is_valid_user_name } from "bencher_valid";
import { validate_jwt, validate_string } from "../../site/util";

const AUTH_FIELDS = {
  username: {
    label: "Name",
    type: "text",
    placeholder: "Full Name",
    icon: "fas fa-user",
    help: "May only use: letters, numbers, contained spaces, apostrophes, periods, commas, and dashes",
    validate: (input) => validate_string(input, is_valid_user_name),
  },
  email: {
    label: "Email",
    type: "email",
    placeholder: "email@example.com",
    icon: "fas fa-envelope",
    help: "Must be a valid email address",
    validate: (input) => validate_string(input, is_valid_email),
  },
  consent: {
    label: "I Agree",
    type: "checkbox",
    placeholder: (
      <small>
        {" "}
        I agree to the{" "}
        <a href="/legal/terms-of-use" target="_blank">
          terms of use
        </a>
        ,{" "}
        <a href="/legal/privacy" target="_blank">
          privacy policy
        </a>
        , and{" "}
        <a href="/legal/license" target="_blank">
          license agreement
        </a>
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
    validate: validate_jwt,
  },
};

export default AUTH_FIELDS;
